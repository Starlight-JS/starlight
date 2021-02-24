use std::{
    collections::VecDeque,
    mem::{size_of, MaybeUninit},
    ptr::{null_mut, NonNull},
};

use self::{
    block_set::BlockSet,
    cell::{GcCell, GcPointer, GcPointerBase, WeakRef, WeakSlot, WeakState},
    precise_allocation::PreciseAllocation,
};
use crate::utils::ordered_set::OrderedSet;
use block::*;
use intrusive_collections::{LinkedList, UnsafeRef};
use wtf_rs::keep_on_stack;
pub mod block;
pub mod block_set;
pub mod cell;
pub mod precise_allocation;
pub mod tiny_bloom_filter;
pub const SIZE_CLASSES: [usize; 15] = [
    16, 24, 32, 48, 64, 96, 128, 256, 512, 768, 1024, 1562, 2048, 3172, 4096,
];

macro_rules! smatch {
    ($size: ident; $($sz: expr => $ix: expr),*) => {
        match $size {
            $($size if $size <= $sz => Some($ix),)+
            _ => None
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SweepResult {
    Recyclable,
    Full,
    Free,
}

#[inline(always)]
pub fn size_class_index_for(size: usize) -> Option<usize> {
    smatch!(size;
        16=>0,
        24=>1,
        32=>2,
        48=>3,
        64=>4,
        96=>5,
        128=>6,
        256=>7,
        512=>8,
        768=>9,
        1024=>10,
        1562=>11,
        2048=>12,
        3172=>13,
        4096=>14
    )
}

#[cfg(not(feature = "valgrind-gc"))]
pub struct SmallArena {
    /// # Free blocks
    ///
    /// Fully free blocks.
    ///
    free_blocks: LinkedList<BlockAdapter>,
    /// # Recyclable blocks
    ///
    /// This list stores all blocks that has at least one freelist entry.
    recyclable_blocks: LinkedList<BlockAdapter>,
    /// # Unavailable blocks
    ///
    /// Block becomes unavailable when its freelist is empty.
    unavailbe_blocks: LinkedList<BlockAdapter>,
    /// Current block where this arena allocates.
    current: *mut BlockHeader,
    /// Arena's cell size. All pointers returned by this arena always have `cell_size` free bytes available.
    cell_size: usize,
}

#[cfg(not(feature = "valgrind-gc"))]
impl SmallArena {
    pub fn new(cell_size: usize) -> Self {
        Self {
            cell_size,
            current: null_mut(),
            unavailbe_blocks: LinkedList::new(BlockAdapter::new()),
            recyclable_blocks: LinkedList::new(BlockAdapter::new()),
            free_blocks: LinkedList::new(BlockAdapter::new()),
        }
    }
    /// Try to steal free block from this arena. Stealing is used when another arena has exceeded its
    /// available blocks and to not allocate memory for block we can just try to steal it.
    pub fn try_steal(&mut self) -> *mut BlockHeader {
        self.free_blocks
            .pop_front()
            .map(UnsafeRef::into_raw)
            .unwrap_or(null_mut())
    }
    #[inline]
    pub fn allocate(&mut self, space: &mut Heap) -> *mut u8 {
        unsafe {
            if self.current.is_null() {
                return self.allocate_slow(space);
            }
            let addr = (*self.current).allocate();
            if addr.is_null() {
                return self.allocate_slow(space);
            }

            addr
        }
    }
    #[inline(never)]
    unsafe fn allocate_slow(&mut self, space: &mut Heap) -> *mut u8 {
        if !self.current.is_null() {
            self.unavailbe_blocks
                .push_back(UnsafeRef::from_raw(self.current));
        }
        if let Some(block) = self.recyclable_blocks.pop_front() {
            let block = UnsafeRef::into_raw(block);
            let p = (*block).allocate();
            if !p.is_null() {
                self.current = block;
                return p;
            }
        }
        if let Some(block) = self.free_blocks.pop_front() {
            let block = UnsafeRef::into_raw(block);
            self.current = block;
            return (*block).allocate();
        }
        let block = space.try_steal(self);
        if !block.is_null() {
            self.current = (*block).header();
            return (*block).header().allocate();
        }
        let block = Block::new(self.cell_size, space);
        self.current = block;
        space.block_set.add(block.block);
        block.allocate()
    }
    /// Sweep arena blocks and push them to correct lists.
    ///
    /// This function will pop blocks from `unavailable_blocks`, `recyclable_blocks` and `current`
    /// and perform sweep on each of these blocks,after sweep they're pushed to recyclable or unavailable
    /// or free blocks.

    pub fn sweep(&mut self) {
        let mut recyclable_blocks = LinkedList::new(BlockAdapter::new());
        let mut unavailable_blocks = LinkedList::new(BlockAdapter::new());
        let mut free_blocks = self.free_blocks.take();

        unsafe {
            while let Some(block) = self.unavailbe_blocks.pop_front() {
                let block = UnsafeRef::into_raw(block);
                match BlockHeader::sweep(&mut *block) {
                    SweepResult::Free => {
                        free_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                    SweepResult::Full => {
                        unavailable_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                    SweepResult::Recyclable => {
                        recyclable_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                }
            }
            if !self.current.is_null() {
                self.recyclable_blocks
                    .push_back(UnsafeRef::from_raw(self.current));
                self.current = null_mut();
            }
            while let Some(block) = self.recyclable_blocks.pop_front() {
                let block = UnsafeRef::into_raw(block);
                match BlockHeader::sweep(&mut *block) {
                    SweepResult::Free => {
                        free_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                    SweepResult::Full => {
                        unavailable_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                    SweepResult::Recyclable => {
                        recyclable_blocks.push_back(UnsafeRef::from_raw(block));
                    }
                }
            }
        }
        self.free_blocks = free_blocks;
        self.unavailbe_blocks = unavailable_blocks;
        self.recyclable_blocks = recyclable_blocks;
    }
}

pub struct SlotVisitor {
    queue: VecDeque<*mut GcPointerBase>,
    bytes_visited: usize,
    sp: usize,
    cons_roots: Vec<(usize, usize)>,
}
pub fn usable_size<T: GcCell>(value: GcPointer<T>) -> usize {
    unsafe {
        let base = value.base.as_ptr();
        if (*base).is_precise_allocation() {
            return (*(*base).precise_allocation()).cell_size as usize;
        }
        let block = Block::from_cell(value.base.as_ptr().cast());
        (*block).header().cell_size as usize
    }
}
impl SlotVisitor {
    unsafe fn visit_raw(&mut self, base: *mut GcPointerBase) {
        if (*base).is_marked() {
            return;
        }

        (*base).mark();
        self.bytes_visited += if (*base).is_precise_allocation() {
            (*(*base).precise_allocation()).cell_size as usize
        } else {
            (*Block::from_cell(base.cast())).header().cell_size as usize
        };
        self.queue.push_back(base);
    }
    pub fn visit<T: GcCell>(&mut self, value: GcPointer<T>) {
        unsafe {
            let base = value.base.as_ptr();
            if (*base).is_marked() {
                return;
            }

            (*base).mark();
            self.bytes_visited += usable_size(value);
            self.queue.push_back(base);
        }
    }
    pub fn add_conservative_roots(&mut self, from: usize, to: usize) {
        self.cons_roots.push((from, to));
    }
    pub fn visit_weak<T: GcCell>(&mut self, slot: &WeakRef<T>) {
        unsafe {
            let inner = &mut *slot.inner.as_ptr();
            inner.state = WeakState::Mark;
        }
    }
}

pub trait MarkingConstraint {
    fn name(&self) -> &str {
        "<anonymous name>"
    }
    fn execute(&mut self, marking: &mut SlotVisitor);
}

pub struct SimpleMarkingConstraint {
    name: String,
    exec: Box<dyn FnMut(&mut SlotVisitor)>,
}
impl SimpleMarkingConstraint {
    pub fn new(name: &str, exec: impl FnMut(&mut SlotVisitor) + 'static) -> Self {
        Self {
            name: name.to_owned(),
            exec: Box::new(exec),
        }
    }
}
impl MarkingConstraint for SimpleMarkingConstraint {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, marking: &mut SlotVisitor) {
        (self.exec)(marking);
    }
}

pub struct Heap {
    large: OrderedSet<*mut PreciseAllocation>,
    arenas: [*mut SmallArena; SIZE_CLASSES.len()],
    weak_slots: std::collections::LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    block_set: BlockSet,
    sp: usize,
    defers: usize,
    allocated: usize,
    max_heap_size: usize,
}

impl Heap {
    pub fn new_weak<T: GcCell>(&mut self, p: GcPointer<T>) -> WeakRef<T> {
        let slot = WeakSlot {
            value: p.base.as_ptr(),
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        unsafe {
            WeakRef {
                inner: NonNull::new_unchecked(self.weak_slots.back().unwrap() as *const _ as *mut _),
                marker: Default::default(),
            }
        }
    }
    pub fn new() -> Self {
        let mut this = Self {
            large: OrderedSet::new(),
            arenas: [null_mut(); SIZE_CLASSES.len()],
            weak_slots: Default::default(),
            constraints: vec![],
            block_set: BlockSet::new(),
            sp: 0,
            defers: 0,
            allocated: 0,
            max_heap_size: 32 * 1024,
        };

        this.init_arenas();
        this.add_constraint(SimpleMarkingConstraint::new("thread roots", |visitor| {
            visitor.add_conservative_roots(
                visitor.sp,
                crate::vm::thread::THREAD.with(|th| th.bounds.origin as usize),
            );
        }));
        this
    }
    pub fn add_constraint(&mut self, constraint: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(constraint));
    }
    fn init_arenas(&mut self) {
        for i in 0..SIZE_CLASSES.len() {
            let sz = SIZE_CLASSES[i];
            self.arenas[i] = Box::into_raw(Box::new(SmallArena::new(sz)));
        }
    }
    pub fn collect_if_necessary(&mut self) {
        if self.allocated >= self.max_heap_size {
            self.gc();
        }
    }
    #[inline(always)]
    unsafe fn allocate_fast(&mut self, size: usize) -> *mut u8 {
        let ix = size_class_index_for(size).unwrap();
        let arena = &mut **self.arenas.get_unchecked(ix);
        self.allocated += arena.cell_size;
        arena.allocate(self)
    }
    #[inline(never)]
    unsafe fn allocate_slow(&mut self, size: usize) -> *mut u8 {
        let precise = PreciseAllocation::try_create(size, self);
        self.large.insert(precise);
        self.allocated = (*precise).cell_size();
        (*precise).cell().cast()
    }
    #[inline]
    pub fn allocate<T: GcCell>(&mut self, value: T) -> GcPointer<T> {
        self.collect_if_necessary();

        let real_size = value.compute_size() + size_of::<GcPointerBase>();
        unsafe {
            let pointer = if real_size <= 4096 {
                self.allocate_fast(real_size)
            } else {
                self.allocate_slow(real_size)
            }
            .cast::<GcPointerBase>();
            let vtable = std::mem::transmute::<_, mopa::TraitObject>(&value as &dyn GcCell).vtable;
            pointer.write(GcPointerBase::new(vtable as _));
            (*pointer).data::<T>().write(value);
            (*pointer).live();
            GcPointer {
                base: NonNull::new_unchecked(pointer),
                marker: Default::default(),
            }
        }
    }
    pub fn defer(&mut self) {
        self.defers += 1;
    }

    pub fn undefer(&mut self) {
        self.defers = self
            .defers
            .checked_sub(1)
            .expect("Trying to undefer non deferred GC");
    }
    #[inline(never)]
    pub fn gc(&mut self) {
        unsafe {
            let mut jmpbuf: MaybeUninit<setjmp::jmp_buf> = MaybeUninit::uninit();
            setjmp::setjmp(jmpbuf.as_mut_ptr());
        }
        let x = self as *const Self as usize;
        keep_on_stack!(&x);
        self.collect_internal(&x);
    }

    pub fn allocated(&self) -> usize {
        self.allocated
    }

    pub fn threshold(&self) -> usize {
        self.max_heap_size
    }
    #[inline(never)]
    fn collect_internal(&mut self, _sp: *const usize) {
        fn current_stack_pointer() -> usize {
            let mut sp: usize = 0;
            sp = &sp as *const usize as usize;
            sp
        }
        self.sp = current_stack_pointer();
        if self.defers > 0 {
            return;
        }
        let mut visitor = SlotVisitor {
            bytes_visited: 0,
            cons_roots: vec![],
            queue: VecDeque::new(),
            sp: self.sp,
        };
        unsafe {
            self.process_roots(&mut visitor);
            self.process_worklist(&mut visitor);
            self.update_weak_references();
            self.reset_weak_references();
            for arena in self.arenas.iter() {
                let arena = &mut **arena;
                arena.sweep();
            }

            self.large.retain(|pcell| {
                let cell = (**pcell).cell();
                if (*cell).is_marked() {
                    (*cell).unmark();
                    true
                } else {
                    std::ptr::drop_in_place((*cell).get_dyn());
                    PreciseAllocation::destroy(&mut **pcell);
                    false
                }
            });
            self.allocated = visitor.bytes_visited;
            if self.allocated > self.max_heap_size {
                self.max_heap_size = (self.allocated as f64 * 1.6f64).floor() as usize;
            }
        }
    }
    fn try_steal(&mut self, into: *mut SmallArena) -> *mut Block {
        for arena in self.arenas.iter() {
            unsafe {
                let arena = &mut **arena;
                if arena as *mut _ != into {
                    let block = arena.free_blocks.pop_back();
                    if let Some(block) = block {
                        return (*UnsafeRef::into_raw(block)).block;
                    }
                }
            }
        }
        null_mut()
    }
    fn update_weak_references(&mut self) {
        for slot in self.weak_slots.iter_mut() {
            match slot.state {
                cell::WeakState::Free => { /* no-op */ }
                cell::WeakState::Unmarked => {
                    slot.value = null_mut();
                    slot.state = cell::WeakState::Free;
                }
                cell::WeakState::Mark => {
                    if slot.value.is_null() {
                        continue;
                    }
                    unsafe {
                        let cell = &*slot.value;

                        if !cell.is_marked() {
                            slot.value = null_mut();
                        }
                    }
                }
            }
        }
    }

    fn reset_weak_references(&mut self) {
        for slot in self.weak_slots.iter_mut() {
            if slot.state == WeakState::Mark {
                slot.state = WeakState::Free;
            }
        }
    }
    fn process_roots(&mut self, visitor: &mut SlotVisitor) {
        unsafe {
            let mut constraints = std::mem::replace(&mut self.constraints, vec![]);
            for constraint in constraints.iter_mut() {
                constraint.execute(visitor);
            }
            std::mem::swap(&mut self.constraints, &mut constraints);

            while let Some((from, to)) = visitor.cons_roots.pop() {
                let mut scan = from as *mut *mut u8;
                let mut to = to as *mut *mut u8;
                if scan > to {
                    std::mem::swap(&mut scan, &mut to);
                }

                while scan < to {
                    let ptr = *scan;
                    if ptr.is_null() {
                        scan = scan.add(1);
                        continue;
                    }

                    self.find_gc_object_pointer_for_marking(ptr, |_, ptr| {
                        visitor.visit_raw(ptr);
                    });
                    scan = scan.add(1);
                }
            }
        }
    }

    fn process_worklist(&mut self, visitor: &mut SlotVisitor) {
        while let Some(ptr) = visitor.queue.pop_front() {
            unsafe {
                (*ptr).get_dyn().trace(visitor);
            }
        }
    }
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut GcPointerBase),
    ) {
        if !self.large.is_empty() {
            if (**self.large.first().unwrap()).above_lower_bound(ptr.cast())
                && (**self.large.last().unwrap()).below_upper_bound(ptr.cast())
            {
                let result = self.large.binary_search(&(ptr as *mut PreciseAllocation));
                match result {
                    Ok(_) => {
                        f(self, ptr.cast());
                    }
                    _ => (),
                }
            }
        }
        let filter = self.block_set.filter;
        let set = &self.block_set.set;
        let candidate = Block::from_cell(ptr);

        if filter.rule_out(candidate as _) {
            return;
        }

        if !set.contains(&candidate) {
            return;
        }

        let mut try_ptr = |p: *mut GcPointerBase| {
            if !(*p).is_live() {
                return false;
            }
            let live = (*candidate).header().is_live(p.cast());
            if live {
                f(self, p.cast());
            }
            live
        };

        if try_ptr(ptr.cast()) {
            return;
        }
        let aligned = (*candidate).header().cell_align(ptr.cast());
        try_ptr(aligned as *mut _);
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        for arena in self.arenas.iter() {
            unsafe {
                let _ = Box::from_raw(*arena);
            }
        }

        for p in self.large.iter() {
            unsafe {
                PreciseAllocation::destroy(&mut **p);
            }
        }
    }
}

impl Drop for SmallArena {
    fn drop(&mut self) {
        unsafe {
            if !self.current.is_null() {
                Block::destroy(&mut *(*self.current).block);
            }

            while let Some(block) = self.unavailbe_blocks.pop_back() {
                let block = UnsafeRef::into_raw(block);
                (*(*block).block).destroy();
            }
            while let Some(block) = self.free_blocks.pop_back() {
                let block = UnsafeRef::into_raw(block);
                (*(*block).block).destroy();
            }

            while let Some(block) = self.recyclable_blocks.pop_back() {
                let block = UnsafeRef::into_raw(block);
                (*(*block).block).destroy();
            }
        }
    }
}
