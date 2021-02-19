pub const SIZE_CLASSES: [usize; 20] = [
    16, 32, 48, 64, 80, 112, 128, 160, 224, 256, 320, 448, 624, 896, 1024, 1360, 2032, 2720, 4080,
    96,
];
macro_rules! smatch {
    ($size: ident; $($sz: expr => $ix: expr),*) => {
        match $size {
            $($size if $size <= $sz => Some($ix),)+
            _ => None
        }
    };
}
fn allocation_size<T: Cell>(val: &T) -> usize {
    /// Align address upwards.
    ///
    /// Returns the smallest x with alignment `align` so that x >= addr.
    /// The alignment must be a power of 2.
    pub fn align_up(addr: u64, align: u64) -> u64 {
        assert!(align.is_power_of_two(), "`align` must be a power of two");
        let align_mask = align - 1;
        if addr & align_mask == 0 {
            addr // already aligned
        } else {
            (addr | align_mask) + 1
        }
    }
    align_up(val.compute_size() as u64 + size_of::<Header>() as u64, 16) as usize
    // round_up_to_multiple_of(16, val.compute_size() + size_of::<Header>())
}
pub fn size_class_index_for(size: usize) -> Option<usize> {
    smatch!(size;
        16 => 0,
        32 => 1,
        48 => 2,
        64 => 3,
        80 => 4,
        112 => 5,
        128 => 6,
        160 => 7,
        224 => 8,
        256 => 9,
        320 => 10,
        448 => 11,
        624 => 12,
        896 => 13,
        1024 => 14,
        1360 => 15,
        2032 => 16,
        2720 => 17,
        4080 => 18,
        96 => 19
    )
}

use std::{
    alloc::System,
    collections::VecDeque,
    mem::size_of,
    ptr::{null_mut, NonNull},
};

use crate::heap::{
    addr::{round_up_to_multiple_of, Address},
    cell::{object_ty_of, Cell, Gc, Header, Tracer, GC_MARKED, GC_UNMARKED, GC_WHITE},
    constraint::MarkingConstraint,
    precise_allocation::PreciseAllocation,
};
pub struct ConservativeRoots {
    pub scan: Vec<(*mut u8, *mut u8)>,
}

use super::{block::*, block_set::BlockSet, handle::HandleTrait};

use dlmalloc::Dlmalloc;
use hashbrown::HashSet;
use intrusive_collections::{LinkedList, UnsafeRef};
use wtf_rs::keep_on_stack;
#[cfg(not(miri))]
use wtf_rs::stack_bounds::StackBounds;

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
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
    current: *mut HeapBlock,
    /// Arena's cell size. All pointers returned by this arena always have `cell_size` free bytes available.
    cell_size: usize,
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
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
    pub fn try_steal(&mut self) -> *mut HeapBlock {
        self.free_blocks
            .pop_front()
            .map(UnsafeRef::into_raw)
            .unwrap_or(null_mut())
    }

    pub fn allocate(&mut self, space: &mut Heap) -> Address {
        unsafe {
            if self.current.is_null() {
                return self.allocate_slow(space);
            }
            let addr = (*self.current).allocate();
            if addr.is_null() {
                return self.allocate_slow(space);
            }

            Address::from_ptr(addr)
        }
    }

    unsafe fn allocate_slow(&mut self, space: &mut Heap) -> Address {
        if !self.current.is_null() {
            self.unavailbe_blocks
                .push_back(UnsafeRef::from_raw(self.current));
        }
        if let Some(block) = self.recyclable_blocks.pop_front() {
            let block = UnsafeRef::into_raw(block);
            let p = (*block).allocate();
            if !p.is_null() {
                self.current = block;
                return Address::from_ptr(p);
            }
        }
        if let Some(block) = self.free_blocks.pop_front() {
            let block = UnsafeRef::into_raw(block);
            self.current = block;
            return Address::from_ptr((*block).allocate());
        }
        let block = HeapBlock::create_with_cell_size(space, self.cell_size).as_ptr();
        self.current = block;
        //space.block_set.add(block);
        Address::from_ptr((*block).allocate())
    }
    /// Sweep arena blocks and push them to correct listsl.
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
                match HeapBlock::sweep(block) {
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
                match HeapBlock::sweep(block) {
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
#[cfg(feature = "valgrind-gc")]
impl Heap {}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
pub struct Heap {
    constraints: Vec<Box<dyn MarkingConstraint>>,

    ndefers: u32,
    max_heap_size: usize,
    allocated: usize,
    pub(super) handles: HashSet<*mut dyn HandleTrait>,
    alloc: dlmalloc::Dlmalloc,
    list: *mut Header,
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
impl Heap {
    pub fn new() -> Box<Self> {
        let mut this = Box::new(Self {
            constraints: vec![],
            handles: Default::default(),
            list: null_mut(),
            ndefers: 0,
            max_heap_size: 64 * 1024,
            allocated: 0,
            alloc: Dlmalloc::new(),
        });
        this.add_core_constraints();
        this.init_arenas();
        this
    }
    fn init_arenas(&mut self) {
        /*    for i in 0..SIZE_CLASSES.len() {
            self.arenas[i] = Box::into_raw(Box::new(SmallArena::new(SIZE_CLASSES[i])));
        }*/
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn test_and_set_marked(cell: *mut Header) -> bool {
        unsafe {
            if (*cell).tag() == GC_UNMARKED {
                (*cell).set_tag(GC_MARKED);
                true
            } else {
                false
            }
        }
    }

    pub fn add_constraint(&mut self, x: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(x));
    }
    fn add_core_constraints(&mut self) {
        /* // we do not want to mark stack when running MIRI.
        #[cfg(not(miri))]
        self.add_constraint(SimpleMarkingConstraint::new(
            "Conservative Roots",
            |marking| {
                let origin = marking.gc.stack_bounds.origin;
                marking.add_conservative_roots(origin, marking.gc.sp as _);
            },
        ));*/
    }

    unsafe fn gc_internal(&mut self, dummy: *const usize) {
        if self.ndefers > 0 {
            return;
        }

        let mut task = Marking {
            gc: self,
            bytes_visited: 0,
            worklist: VecDeque::with_capacity(8),
            cons: ConservativeRoots {
                scan: Vec::with_capacity(2),
            },
            file: None,
        };

        task.run();

        let visited = task.bytes_visited;
        drop(task);
        let mut prev = null_mut();
        let mut cur = self.list;
        self.allocated = 0;
        while !cur.is_null() {
            if (*cur).tag() == GC_MARKED {
                prev = cur;
                cur = (*cur).next;
                (*prev).set_tag(GC_UNMARKED);
                self.allocated += (*prev).get_dyn().compute_size() + core::mem::size_of::<Header>();
            } else {
                let unreached = cur;
                cur = (*cur).next;
                if !prev.is_null() {
                    (*prev).next = cur;
                } else {
                    self.list = cur;
                }
                std::ptr::drop_in_place((*unreached).get_dyn());
                self.alloc.free(unreached.cast(), 0, 0);
            }
        }
        //self.allocated = visited;
        if self.allocated >= self.max_heap_size {
            self.max_heap_size = (self.allocated as f64 * 1.7) as usize;
        }
        /*for arena in self.arenas.iter().copied() {
            unsafe {
                (*arena).sweep();
            }
        }

        self.precise_allocations.retain(|alloc| {
            let cell = (**alloc).cell();
            if (*cell).tag() == GC_WHITE {
                (**alloc).destroy();
                false
            } else {
                (*cell).set_tag(GC_WHITE);
                true
            }
        });
        self.precise_allocations.sort_unstable();*/
        self.allocated = visited;
        self.max_heap_size = (visited as f64 * 1.7) as usize;
    }

    pub fn gc(&mut self) {
        let x = 0;
        keep_on_stack!(&x);
        unsafe {
            self.gc_internal(&x);
        }
    }

    pub fn collect_if_necessary(&mut self) {
        if self.allocated <= self.max_heap_size {
            return;
        }
        self.gc();
    }
    pub fn defer_gc(&mut self) {
        self.ndefers += 1;
    }
    pub fn undefer_gc(&mut self) {
        self.ndefers -= 1;
    }
    #[inline(never)]
    unsafe fn alloc_slow(&mut self, size: usize) -> Address {
        /*assert!(size > 4080);
        let ix = self.precise_allocations.len();
        let precise = PreciseAllocation::try_create(self, size, ix as _);
        self.precise_allocations.push(precise);
        Address::from_ptr((*precise).cell())*/
        unreachable!()
    }

    /// Allocate `size` bytes in GC heap.
    ///
    /// # Safety
    ///
    /// This function is unsafe since it returns partially initialized data.
    /// Only first 8 bytes is initialized with GC object header.
    ///
    ///
    #[inline]
    pub unsafe fn allocate_raw(&mut self, size: usize) -> Address {
        self.collect_if_necessary();
        self.allocated += size;
        /*if size > 4080 {
            self.alloc_slow(size)
        } else {
            let arena = self.arenas[size_class_index_for(size).unwrap()];
            (*arena).allocate(self)
        }*/
        Address::from_ptr(self.alloc.malloc(size, 16))
    }
    pub fn heap_usage(&self) -> usize {
        self.allocated
    }

    pub fn alloc<T: Cell>(&mut self, value: T) -> Gc<T> {
        unsafe {
            let size = allocation_size(&value);
            let memory = self.allocate_raw(size).to_mut_ptr::<Header>();
            assert!(!memory.is_null());

            memory.write(Header::new(self, null_mut(), object_ty_of(&value)));
            (*memory).set_tag(GC_WHITE);
            let sz = value.compute_size();
            (*memory).data_start().to_mut_ptr::<T>().write(value);
            /*std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                (*memory).data_start().to_mut_ptr::<u8>(),
                sz,
            );*/
            //std::mem::forget(value);
            #[cfg(feature = "valgrind-gc")]
            {
                println!(
                    "Alloc {:p} ({}): {}",
                    memory,
                    std::any::type_name::<T>(),
                    std::backtrace::Backtrace::capture()
                );
            }
            (*memory).next = self.list;
            self.list = memory;
            Gc {
                cell: NonNull::new_unchecked(memory),
                marker: Default::default(),
            }
        }
    }
}
pub struct Marking<'a> {
    pub gc: &'a mut Heap,
    pub worklist: VecDeque<*mut Header>,
    pub bytes_visited: usize,
    cons: ConservativeRoots,
    #[allow(dead_code)]
    file: Option<&'a mut std::fs::File>,
}

impl<'a> Marking<'a> {
    pub fn run(&mut self) {
        self.process_constraints();
        self.process_roots();
        self.process_worklist();
    }
    fn process_constraints(&mut self) {
        unsafe {
            let mut constraints = vec![];
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
            for c in constraints.iter_mut() {
                c.execute(self);
            }
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
        }
    }
    fn process_roots(&mut self) {
        unsafe {
            /*let mut head = self.gc.scopes;
            while !head.is_null() {
                let scope = &mut *head;
                scope.roots.retain(|item| {
                    /*if item.is_null() {
                        false
                    } else {
                        self.mark(*item);
                        true
                    }*/
                    match item {
                        Some(ptr) => {
                            (*ptr.as_ptr()).trace(self);
                            true
                        }
                        None => false,
                    }
                });
                head = (*head).next;
            }*/
            let this = self as *mut Self;
            for handle in self.gc.handles.iter().copied() {
                (*handle).trace(&mut *this);
            }
        }
    }
    fn process_worklist(&mut self) {
        while let Some(item) = self.worklist.pop_front() {
            unsafe {
                self.visit_value(item);
            }
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn mark(&mut self, val: *mut Header) {
        unsafe {
            if Heap::test_and_set_marked(val) {
                let obj = val;
                //println!("{}", obj.get_dyn().get_typename());
                self.bytes_visited += round_up_to_multiple_of(
                    16,
                    (*obj).get_dyn().compute_size() + core::mem::size_of::<Header>(),
                );
                self.worklist.push_back(obj);
            }
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn visit_value(&mut self, val: *mut Header) {
        unsafe {
            (*val).get_dyn().trace(self);
        }
    }
    /*pub fn add_conservative_roots(&mut self, from: *mut u8, to: *mut u8) {
        self.cons.scan.push((from, to));
    }

    #[allow(clippy::mutable_key_type)]
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut Header),
    ) {
        if !self.gc.precise_allocations.is_empty() {
            if (**self.gc.precise_allocations.first().unwrap()).above_lower_bound(ptr.cast())
                && (**self.gc.precise_allocations.last().unwrap()).below_upper_bound(ptr.cast())
            {
                let result = self
                    .gc
                    .precise_allocations
                    .binary_search(&PreciseAllocation::from_cell(ptr.cast()));
                match result {
                    Ok(ix) => {
                        if (*self.gc.precise_allocations[ix]).has_valid_cell {
                            f(self, ptr.cast());
                        }
                    }
                    _ => (),
                }
            }
        }
        let filter = self.gc.block_set.filter;
        let set = &self.gc.block_set.set;
        let candidate = HeapBlock::from_cell(ptr.cast());
        if filter.rule_out(candidate as _) {
            return;
        }

        if !set.contains(&candidate) {
            return;
        }

        let mut try_ptr = |ptr| {
            let is_live = (*candidate).cell_from_possible_pointer(Address::from_ptr(ptr));
            if !is_live.is_null() && !(*is_live).is_zapped() {
                f(self, ptr as *mut _);
                true
            } else {
                false
            }
        };

        if try_ptr(ptr) {
            return;
        }
    }*/
}

impl<'a> Tracer for Marking<'a> {
    fn trace(&mut self, hdr: *mut Header) {
        self.mark(hdr);
    }
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            let mut object = self.list;
            while !object.is_null() {
                let obj = object;
                object = (*obj).next;
                std::ptr::drop_in_place((*obj).get_dyn());
                libc::free(obj.cast());
            }
            self.constraints.clear();
        }
    }
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
impl Drop for SmallArena {
    fn drop(&mut self) {
        unsafe {
            if !self.current.is_null() {
                HeapBlock::destroy(self.current);
            }

            while let Some(b) = self.unavailbe_blocks.pop_front() {
                HeapBlock::destroy(UnsafeRef::into_raw(b));
            }

            while let Some(b) = self.recyclable_blocks.pop_front() {
                HeapBlock::destroy(UnsafeRef::into_raw(b));
            }
            while let Some(b) = self.free_blocks.pop_front() {
                HeapBlock::destroy(UnsafeRef::into_raw(b));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use crate::heap::cell::{Cell, Trace};

    use super::Heap;
    struct Foo;
    static FOO: AtomicU32 = AtomicU32::new(0);

    impl Drop for Foo {
        fn drop(&mut self) {
            FOO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    unsafe impl Trace for Foo {}
    impl Cell for Foo {}

    #[test]
    fn test_gc_root() {
        let mut heap = Heap::new();

        let x = heap.alloc(Foo).root(&mut heap);
        let y = heap.alloc(Foo);
        heap.gc();
        assert_eq!(FOO.load(std::sync::atomic::Ordering::Relaxed), 1);
        drop(x);
        drop(y);
    }

    struct FooLarge([u8; 8192]);
    static FOO_LARGE: AtomicU32 = AtomicU32::new(0);

    impl Drop for FooLarge {
        fn drop(&mut self) {
            FOO_LARGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    unsafe impl Trace for FooLarge {}
    impl Cell for FooLarge {}

    #[test]
    fn test_gc_large_root() {
        let mut heap = Heap::new();

        let x = heap.alloc(FooLarge([0; 8192])).root(&mut heap);
        let y = heap.alloc(FooLarge([0; 8192]));
        heap.gc();
        assert_eq!(FOO_LARGE.load(std::sync::atomic::Ordering::Relaxed), 1);
        drop(x);
        drop(y);
    }
}

#[cfg(any(target_os = "windows", feature = "valgrind-gc"))]
pub struct Heap {
    constraints: Vec<Box<dyn MarkingConstraint>>,
    ndefers: u32,
    max_heap_size: usize,
    allocated: usize,
    pub(super) handles: HashSet<*mut dyn HandleTrait>,
    list: *mut Header,
}

#[cfg(any(target_os = "windows", feature = "valgrind-gc"))]
impl Heap {
    pub fn defer_gc(&mut self) {
        self.ndefers += 1;
    }
    pub fn undefer_gc(&mut self) {
        self.ndefers -= 1;
    }

    pub fn heap_usage(&self) -> usize {
        self.allocated
    }

    pub fn alloc<T: Cell>(&mut self, value: T) -> Gc<T> {
        unsafe {
            if self.allocated > self.max_heap_size {
                self.gc();
            }

            let size = allocation_size(&value);
            self.allocated += size;
            let memory = libc::malloc(size).cast::<Header>();
            assert!(!memory.is_null());

            memory.write(Header::new(self, null_mut(), object_ty_of(&value)));
            (*memory).set_tag(GC_UNMARKED);
            let sz = value.compute_size();
            (*memory).data_start().to_mut_ptr::<T>().write(value);
            /*std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                (*memory).data_start().to_mut_ptr::<u8>(),
                sz,
            );*/
            //std::mem::forget(value);
            (*memory).next = self.list;
            self.list = memory;
            Gc {
                cell: NonNull::new_unchecked(memory),
                marker: Default::default(),
            }
        }
    }

    pub fn gc(&mut self) {
        unsafe {
            if self.ndefers > 0 {
                return;
            }
            //self.sp = dummy as usize;

            let mut task = Marking {
                gc: self,
                bytes_visited: 0,
                worklist: VecDeque::with_capacity(8),
                cons: ConservativeRoots {
                    scan: Vec::with_capacity(2),
                },
                file: None,
            };

            task.run();

            let visited = task.bytes_visited;
            drop(task);
            let mut prev = null_mut();
            let mut cur = self.list;
            self.allocated = 0;
            while !cur.is_null() {
                if (*cur).tag() == GC_MARKED {
                    prev = cur;
                    cur = (*cur).next;
                    (*prev).set_tag(GC_UNMARKED);
                    self.allocated +=
                        (*prev).get_dyn().compute_size() + core::mem::size_of::<Header>();
                } else {
                    let unreached = cur;
                    cur = (*cur).next;
                    if !prev.is_null() {
                        (*prev).next = cur;
                    } else {
                        self.list = cur;
                    }
                    std::ptr::drop_in_place((*unreached).get_dyn());
                    libc::free(unreached.cast());
                }
            }
            //self.allocated = visited;
            if self.allocated >= self.max_heap_size {
                self.max_heap_size = (self.allocated as f64 * 1.7) as usize;
            }
        }
    }

    pub fn add_constraint(&mut self, x: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(x));
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn test_and_set_marked(cell: *mut Header) -> bool {
        unsafe {
            if (*cell).tag() == GC_UNMARKED {
                (*cell).set_tag(GC_MARKED);
                true
            } else {
                false
            }
        }
    }

    pub fn new() -> Box<Self> {
        let mut this = Box::new(Self {
            constraints: vec![],
            handles: Default::default(),
            ndefers: 0,
            max_heap_size: 256 * 1024,
            allocated: 0,
            list: null_mut(),
        });

        this
    }
}

#[cfg(any(target_os = "windows", feature = "valgrind-gc"))]
impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            let mut object = self.list;
            while !object.is_null() {
                let obj = object;
                object = (*obj).next;
                std::ptr::drop_in_place((*obj).get_dyn());
                libc::free(obj.cast());
            }
        }
    }
}

impl AsMut<Heap> for Heap {
    fn as_mut(&mut self) -> &mut Heap {
        self
    }
}
