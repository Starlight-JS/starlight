use crate::{gc::jet::block::FreeEntry, vm::GcParams};

use self::{block::HeapBlock, block_allocator::BlockAllocator};
use super::{cell::*, GarbageCollector, GcStats, MarkingConstraint};
use libmimalloc_sys::mi_heap_t;
use std::{
    any::TypeId,
    collections::LinkedList,
    intrinsics::unlikely,
    mem::transmute,
    num::NonZeroU16,
    ptr::{drop_in_place, null_mut, NonNull},
};

pub mod block;
pub mod block_allocator;

pub const JET_FREE: u8 = DEFINETELY_WHITE;
pub const JET_UNMARKED: u8 = POSSIBLY_GREY;
pub const JET_MARKED: u8 = POSSIBLY_BLACK;

pub struct SmallArena {
    pub recycled: *mut HeapBlock,
    pub full: *mut HeapBlock,
    pub current: *mut HeapBlock,
    pub index: usize,
}

pub fn size_to_index(sz: usize) -> usize {
    match sz {
        x if x <= 16 => 0,
        x if x <= 32 => 1,
        x if x <= 48 => 2,
        x if x <= 64 => 3,
        x if x <= 96 => 4,
        x if x <= 128 => 5,
        x if x <= 196 => 6,
        x if x <= 256 => 7,
        x if x <= 512 => 8,
        x if x <= 768 => 9,
        x if x <= 1024 => 10,
        x if x <= 1536 => 11,
        x if x <= 2048 => 12,
        x if x <= 3072 => 13,
        x if x <= 4095 => 14,
        _ => unreachable!(),
    }
}

pub fn index_to_size(ix: usize) -> usize {
    match ix {
        0 => 16,
        1 => 32,
        2 => 48,
        3 => 64,
        4 => 96,
        5 => 128,
        6 => 196,
        7 => 256,
        8 => 512,
        9 => 768,
        10 => 1024,
        11 => 1536,
        12 => 2048,
        13 => 3072,
        14 => 4095,
        _ => unreachable!(),
    }
}

pub struct JetGC {
    block_allocator: BlockAllocator,
    mi_heap: *mut mi_heap_t,
    weak_slots: LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    filter: TinyBloomFilter,
    threshold: usize,
    collect_conservative: bool,
    arenas: [SmallArena; 14],
}

impl JetGC {
    pub fn new(gc_params: GcParams) -> Self {
        Self {
            filter: TinyBloomFilter::new(0),
            collect_conservative: gc_params.conservative_marking,
            arenas: [
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 0,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 1,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 2,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 3,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 4,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 5,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 6,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 7,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 8,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 9,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 10,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 11,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 12,
                },
                SmallArena {
                    recycled: null_mut(),
                    full: null_mut(),
                    current: null_mut(),
                    index: 13,
                },
            ],
            block_allocator: BlockAllocator::new(),
            weak_slots: Default::default(),
            constraints: vec![],
            sp: 0,
            defers: 0,

            allocated: 0,
            threshold: 256 * 1024,
            mi_heap: unsafe { libmimalloc_sys::mi_heap_new() },
        }
    }
    unsafe fn allocate_small(&mut self, size: usize) -> *mut u8 {
        let ix = size_to_index(size);
        let p = self as *mut Self;
        self.allocated += index_to_size(ix);
        let arena = &mut self.arenas[ix];
        if unlikely(arena.current.is_null() || (*arena.current).is_full()) {
            if !arena.current.is_null() {
                (*arena.current).next = arena.full;
                arena.full = arena.current;
            }
            arena.current = if arena.recycled.is_null() {
                self.block_allocator
                    .allocate(NonZeroU16::new_unchecked(index_to_size(ix) as _))
                    as *mut _
            } else {
                let block = arena.recycled;
                arena.recycled = (*block).next;

                block
            };
            (*arena.current).heap = p;
            self.filter.add_bits(arena.current as usize);
        }

        (*arena.current).allocate()
    }
    /// Tries to find GC object for marking at `ptr`.
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut GcPointerBase),
    ) {
        let block = HeapBlock::from_cell(ptr);
        if self.filter.rule_out(block as _) {
            if (*block).heap == self as *mut Self {
                let cell = (*block).cell_from_possible_pointer(ptr);
                if !cell.is_null() && (*cell.cast::<GcPointerBase>()).state() == JET_UNMARKED {
                    f(self, cell.cast());
                    return;
                }
            }
        }
        if libmimalloc_sys::mi_heap_check_owned(self.mi_heap, ptr.cast())
            && libmimalloc_sys::mi_heap_contains_block(self.mi_heap, ptr.cast())
        {
            f(self, ptr.cast());
        }
    }

    #[inline(never)]
    fn collect_internal(&mut self, _sp: *const usize) {
        fn current_stack_pointer() -> usize {
            let mut sp: usize = 0;
            sp = &sp as *const usize as usize;
            sp
        }
        // Capture all the registers to scan them conservatively. Note that this also captures
        // FPU registers too because JS values is NaN boxed and exist in FPU registers.

        // Get stack pointer for scanning thread stack.
        self.sp = current_stack_pointer();
        if self.defers > 0 {
            return;
        }
        let sp = self.sp;
        let mut visitor = SlotVisitor {
            bytes_visited: 0,

            cons_roots: vec![],
            queue: Vec::with_capacity(256),
        };

        unsafe {
            if self.collect_conservative {
                /*#[cfg(not(target_arch = "wasm32"))]
                {
                    let registers = crate::vm::thread::Thread::capture_registers();
                    if !registers.is_empty() {
                        visitor.cons_roots.push((
                            registers.first().unwrap() as *const usize as _,
                            registers.last().unwrap() as *const usize as _,
                        ));
                    }
                }*/
                crate::vm::thread::THREAD.with(|thread| {
                    visitor
                        .cons_roots
                        .push((thread.bounds.origin as _, sp as usize));
                });
            }
            self.process_roots(&mut visitor);

            self.process_worklist(&mut visitor);

            self.update_weak_references();
            self.reset_weak_references();
            #[cfg(feature = "enable-gc-tracking")]
            if self.track_allocations {
                #[cold]
                #[inline(never)]
                unsafe fn cleanup_allocations(this: &mut Heap) {
                    this.allocations.retain(|alloc, info| {
                        if (**alloc).state() == DEFINETELY_WHITE {
                            println!("retain {:p} \n{}", *alloc, info);
                        }
                        (**alloc).state() == POSSIBLY_BLACK
                    });
                }
                cleanup_allocations(self);
            }

            self.allocated = 0;
            for i in 0..self.arenas.len() {
                let arena = &mut *(&mut self.arenas[i] as *mut SmallArena);
                if !arena.current.is_null() {
                    (*arena.current).next = arena.full;
                    arena.full = arena.current;
                }

                arena.current = null_mut();

                let mut sweep = arena.full;
                let later = arena.recycled;
                arena.recycled = null_mut();
                let mut sweep_block = |block: &'static mut HeapBlock| {
                    let mut freelist = null_mut();
                    let mut all_free = true;
                    block.for_each_cell(|cell| {
                        let cell = cell.cast::<GcPointerBase>();
                        if (*cell).state() == JET_UNMARKED {
                            drop_in_place((*cell).get_dyn());
                            (*cell.cast::<FreeEntry>()).next = freelist;
                            freelist = cell.cast();
                            (*cell).force_set_state(JET_FREE);
                        } else if (*cell).state() == JET_FREE {
                            (*cell.cast::<FreeEntry>()).next = freelist;
                            freelist = cell.cast();
                        } else {
                            self.allocated += block.cell_size();
                            all_free = false;
                        }
                    });
                    (*block).freelist = freelist;
                    if all_free {
                        self.block_allocator.free(block as *mut _);
                    } else {
                        block.next = arena.recycled;
                        arena.recycled = block as *mut _;
                    }
                };
                while !sweep.is_null() {
                    let next = (*sweep).next;
                    sweep_block(&mut *sweep);
                    sweep = next;
                }
                sweep = later;
                while !sweep.is_null() {
                    let next = (*sweep).next;
                    sweep_block(&mut *sweep);
                    sweep = next;
                }
            }
            libmimalloc_sys::mi_heap_visit_blocks(
                self.mi_heap,
                true,
                Some(sweep),
                self as *mut Self as _,
            );

            if self.allocated > self.threshold {
                self.threshold = (self.allocated as f64 * 1.5f64) as usize;
            }
            libmimalloc_sys::mi_heap_collect(self.mi_heap, false);
        }
    }
    /// Update all weak references.
    ///
    ///
    /// Algorithm:
    /// ```rust,ignore
    /// for weak_slot in weak_slots {
    ///     if weak_slot.is_marked() {
    ///         if !weak_slot.object.is_marked() {
    ///             weak_slot.object = NULL;
    ///         }
    ///     }
    ///     
    /// }
    ///
    /// ```
    ///
    fn update_weak_references(&mut self) {
        for slot in self.weak_slots.iter_mut() {
            match slot.state {
                WeakState::Free => { /* no-op */ }
                WeakState::Unmarked => {
                    slot.value = null_mut();
                    slot.state = WeakState::Free;
                }
                WeakState::Mark => {
                    if slot.value.is_null() {
                        continue;
                    }

                    unsafe {
                        let cell = &*slot.value;

                        if cell.state() == JET_UNMARKED {
                            slot.value = null_mut();
                        }
                    }
                }
            }
        }
    }
    /// Walk all weak slots and reset them. If slot is free then it is unlinked from slots linked list
    /// otherwise it is just unmarked.
    fn reset_weak_references(&mut self) {
        let mut cursor = self.weak_slots.cursor_front_mut();
        while let Some(item) = cursor.current() {
            if item.state == WeakState::Free {
                cursor.remove_current();
            } else {
                item.state = WeakState::Unmarked;
                cursor.move_next();
            }
        }
    }
    /// This function marks all potential roots.
    ///
    ///
    /// How it works:
    /// - Execute all marking constraints
    /// - Scan pointer ranges that were added to `SlotVisitor` during execution of constraints
    /// for potential root objects.
    fn process_roots(&mut self, visitor: &mut SlotVisitor) {
        unsafe {
            let mut constraints = std::mem::take(&mut self.constraints);
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
                if core::intrinsics::unlikely(self.collect_conservative) {
                    while scan < to {
                        let ptr = *scan;
                        if ptr.is_null() {
                            scan = scan.add(1);
                            continue;
                        }
                        let mut found = false;

                        self.find_gc_object_pointer_for_marking(ptr, |_, mut ptr| {
                            visitor.visit_raw(&mut ptr);
                            found = true;
                        });
                        #[cfg(target_pointer_width = "64")]
                        if !found {
                            let value = transmute::<_, crate::vm::value::JsValue>(ptr);
                            if value.is_object() {
                                self.find_gc_object_pointer_for_marking(
                                    value.get_pointer().cast(),
                                    |_, mut ptr| {
                                        visitor.visit_raw(&mut ptr);
                                    },
                                );
                            }
                        }
                        scan = scan.add(1);
                    }
                }
            }
        }
    }
    /// Process mark stack marking all values from it in LIFO way.
    ///
    ///
    /// This function is very simple:
    ///
    /// ```rust,ignore
    /// while let Some(object) = visitor.queue.pop_front() {
    ///     grey2black(object);
    ///     object.vtable.trace(object,visitor);
    /// }
    /// ```
    ///
    fn process_worklist(&mut self, visitor: &mut SlotVisitor) {
        while let Some(ptr) = visitor.queue.pop() {
            unsafe {
                //(*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
                (*ptr).get_dyn().trace(visitor);
            }
        }
    }
}

impl GarbageCollector for JetGC {
    fn weak_slots(&mut self, cb: &mut dyn FnMut(*mut WeakSlot)) {
        for slot in self.weak_slots.iter() {
            cb(slot as *const _ as *mut _);
        }
    }
    fn stats(&self) -> GcStats {
        GcStats {
            allocated: self.allocated,
            threshold: self.threshold,
        }
    }

    fn add_constraint(&mut self, constraint: Box<dyn MarkingConstraint>) {
        self.constraints.push(constraint);
    }
    fn make_weak_slot(&mut self, p: *mut GcPointerBase) -> *mut WeakSlot {
        let slot = WeakSlot {
            value: p,
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        {
            self.weak_slots.back_mut().unwrap() as *mut _
        }
    }
    fn allocate(
        &mut self,
        size: usize,
        vtable: usize,
        type_id: TypeId,
    ) -> Option<NonNull<GcPointerBase>> {
        unsafe {
            let pointer = if size <= 4095 {
                self.allocate_small(size)
            } else {
                self.allocated += size;
                libmimalloc_sys::mi_heap_malloc_aligned(self.mi_heap, size, 16).cast::<u8>()
            }
            .cast::<GcPointerBase>();

            pointer.write(GcPointerBase::new(vtable, type_id));
            (*pointer).force_set_state(JET_UNMARKED);

            Some(NonNull::new_unchecked(pointer))
        }
    }

    fn defer(&mut self) {
        self.defers += 1;
    }
    fn undefer(&mut self) {
        self.defers = self
            .defers
            .checked_sub(1)
            .expect("Trying to undefer non deferred GC");
    }
    fn collect_if_necessary(&mut self) {
        if self.allocated > self.threshold {
            self.gc();
        }
    }

    fn gc(&mut self) {
        unsafe {
            self.collect_internal(&0);
        }
    }

    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        unsafe extern "C" fn walk(
            _heap: *const libmimalloc_sys::mi_heap_t,
            _area: *const libmimalloc_sys::mi_heap_area_t,
            block: *mut libc::c_void,
            block_sz: usize,
            arg: *mut libc::c_void,
        ) -> bool {
            if block.is_null() {
                return true;
            }
            let closure: *mut dyn FnMut(*mut GcPointerBase, usize) -> bool =
                std::mem::transmute(*arg.cast::<(usize, usize)>());

            (&mut *closure)(block as _, block_sz)
        }

        let f: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool = callback;
        let trait_obj: (usize, usize) = unsafe { std::mem::transmute(f) };
        unsafe {
            libmimalloc_sys::mi_heap_visit_blocks(
                self.mi_heap,
                true,
                Some(walk),
                &trait_obj as *const (usize, usize) as _,
            );
        }

        let mut walk_block = |block: *mut HeapBlock| unsafe {
            (*block).for_each_cell(|cell| {
                let cell = cell.cast::<GcPointerBase>();
                if (*cell).state() != JET_FREE {
                    callback(cell, (*block).cell_size());
                }
            });
        };

        for arena in self.arenas.iter() {
            if !arena.current.is_null() {
                walk_block(arena.current);
            }
            let mut w = arena.full;
            unsafe {
                while !w.is_null() {
                    walk_block(w);
                    w = (*w).next;
                }
                w = arena.recycled;
                while !w.is_null() {
                    walk_block(w);
                    w = (*w).next;
                }
            }
        }
    }
}

/// Visits garbage collected objects
pub struct SlotVisitor {
    pub(super) queue: Vec<*mut GcPointerBase>,
    pub(super) cons_roots: Vec<(usize, usize)>,
    pub(super) bytes_visited: usize,
}

impl Tracer for SlotVisitor {
    fn visit_weak(&mut self, slot: *const WeakSlot) {
        unsafe {
            let inner = &mut *(slot as *mut WeakSlot);
            inner.state = WeakState::Mark;
        }
    }

    fn visit_raw(&mut self, cell: &mut *mut GcPointerBase) -> GcPointer<dyn GcCell> {
        let base = *cell;
        unsafe {
            if !(*base).set_state(JET_UNMARKED, JET_MARKED) {
                return GcPointer {
                    base: NonNull::new_unchecked(base as *mut _),
                    marker: Default::default(),
                };
            }
            self.bytes_visited += 1;
            //prefetch_read_data(base, 1);
            self.queue.push(base as *mut _);
            GcPointer {
                base: NonNull::new_unchecked(base as *mut _),
                marker: Default::default(),
            }
        }
    }

    fn visit(&mut self, cell: &mut GcPointer<dyn GcCell>) -> GcPointer<dyn GcCell> {
        unsafe {
            let base = cell.base.as_ptr();
            if !(*base).set_state(JET_UNMARKED, JET_MARKED) {
                return *cell;
            }
            self.bytes_visited += 1;
            // prefetch_read_data(base, 1);
            self.queue.push(base);
            *cell
        }
    }

    fn add_conservative(&mut self, from: usize, to: usize) {
        self.cons_roots.push((from, to));
    }
}

#[allow(dead_code)]
unsafe extern "C" fn sweep(
    _heap: *const libmimalloc_sys::mi_heap_t,
    _area: *const libmimalloc_sys::mi_heap_area_t,
    block: *mut libc::c_void,
    block_sz: usize,
    arg: *mut libc::c_void,
) -> bool {
    // weird, mimalloc passes NULL pointer at first iteration.
    if block.is_null() {
        return true;
    }
    let gc = &mut *(arg.cast::<JetGC>());
    let ptr = block.cast::<GcPointerBase>();
    if (*ptr).state() == JET_UNMARKED {
        std::ptr::drop_in_place((*ptr).get_dyn());
        libmimalloc_sys::mi_free(ptr.cast());
    } else {
        gc.allocated += block_sz;
        assert!((*ptr).set_state(JET_MARKED, JET_UNMARKED));
    }

    true
}

impl Drop for JetGC {
    fn drop(&mut self) {
        unsafe {
            libmimalloc_sys::mi_heap_visit_blocks(
                self.mi_heap,
                true,
                Some(sweep),
                self as *mut Self as _,
            );
            libmimalloc_sys::mi_heap_destroy(self.mi_heap);
        }
    }
}

#[derive(Copy, Clone)]
pub struct TinyBloomFilter {
    bits: usize,
}
impl TinyBloomFilter {
    pub const fn new(bits: usize) -> Self {
        Self { bits }
    }

    pub fn rule_out(&self, bits: usize) -> bool {
        if bits == 0 {
            return true;
        }
        if (bits & self.bits) != bits {
            return true;
        }
        false
    }

    pub fn add(&mut self, other: &Self) {
        self.bits |= other.bits;
    }

    pub fn add_bits(&mut self, bits: usize) {
        self.bits |= bits;
    }

    pub fn reset(&mut self) {
        self.bits = 0;
    }

    pub fn bits(&self) -> usize {
        self.bits
    }
}
