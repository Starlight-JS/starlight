//! Region based garbage collector.
//!
//!
//! This GC is using Immix algorithm for small enough objects (Fits under 32KB blocks) and mimalloc for others.
//! Allocation for small objects in almost all cases is simple bump-pointer.
//!
//!
//!
//! ## To move or not to move
//!
//! RegionGC does not yet implement moving but we can add it in future. Main problem is performance because we have to track pointers
//! that is found on stack convservatively and can't be moved, and including our `letroot!` functionlaity this makes evacuation almost useless.

use self::{allocator::ImmixSpace, block::ImmixBlock, los::LargeObjectSpace};
use super::{cell::*, Address, GarbageCollector, GcStats, MarkingConstraint};
use std::ptr::null_mut;
use std::ptr::NonNull;
use std::{any::TypeId, collections::LinkedList};
use std::{intrinsics::likely, mem::transmute};
use yastl::Pool;
pub mod allocator;
pub mod block;
pub mod block_allocator;
pub mod los;

pub struct RegionGC {
    threshold: usize,
    allocated: usize,
    los: LargeObjectSpace,
    immix: ImmixSpace,
    weak_slots: LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    n_workers: u32,
    pool: Option<Pool>,
}

impl RegionGC {
    pub fn new(immix_size: usize, n_workers: u32, par_marking: bool) -> RegionGC {
        Self {
            threshold: 100 * 1024 * 1024,
            los: LargeObjectSpace::new(),
            immix: ImmixSpace::new(immix_size),
            weak_slots: LinkedList::new(),
            constraints: Vec::new(),
            sp: 0,
            defers: 0,
            pool: if par_marking {
                Some(Pool::new(n_workers as _))
            } else {
                None
            },
            n_workers,
            allocated: 0,
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
        let registers = crate::vm::thread::Thread::capture_registers();
        // Get stack pointer for scanning thread stack.
        self.sp = current_stack_pointer();
        if self.defers > 0 {
            return;
        }
        let sp = self.sp;
        let mut visitor = SlotVisitor {
            bytes_visited: 0,
            gc: self as *mut Self,
            cons_roots: vec![],
            queue: Vec::with_capacity(256),
        };

        unsafe {
            let mut blocks = self.immix.get_all_blocks();
            for block in blocks.iter() {
                (**block).line_map.clear_all();
            }
            if !registers.is_empty() {
                visitor.cons_roots.push((
                    registers.first().unwrap() as *const usize as _,
                    registers.last().unwrap() as *const usize as _,
                ));
            }
            crate::vm::thread::THREAD.with(|thread| {
                visitor
                    .cons_roots
                    .push((thread.bounds.origin as _, sp as usize));
            });

            self.process_roots(&mut visitor);
            drop(registers);
            if let Some(ref mut pool) = self.pool {
                crate::gc::pmarking::start(&visitor.queue, self.n_workers as _, pool);
            } else {
                self.process_worklist(&mut visitor);
            }
            self.update_weak_references();
            self.reset_weak_references();

            let los = self.los.sweep();

            let immix = self.immix.sweep();

            let mut free = vec![];
            let mut unavail = vec![];
            let mut recyc = vec![];

            while let Some(block) = blocks.pop() {
                if (*block).is_empty() {
                    println!("free {:p}", block);
                    free.push(block);
                } else {
                    let (holes, _) = (*block).count_holes_and_marked_lines();
                    match holes {
                        0 => {
                            println!("unavail {:p}", block);
                            unavail.push(block)
                        }
                        _ => {
                            println!("recyc {:p} #{}", block, holes);
                            recyc.push(block)
                        }
                    }
                }
            }
            self.immix.set_recyclable_blocks(recyc);
            self.immix.return_blocks(free.into_iter());
            self.immix.unavail = unavail;
            self.allocated = los + immix;
            if self.allocated >= self.threshold {
                self.threshold = (self.allocated as f64 * 1.5) as usize;
            }
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

                        if cell.state() == DEFINETELY_WHITE {
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
                (*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
                (*ptr).get_dyn().trace(visitor);
            }
        }
    }
    /// Tries to find GC object for marking at `ptr`.
    ///
    ///
    /// TODO: Interior pointers. Right now this function just checks if `ptr` is a block allocated inside mimalloc gc.
    ///
    ///
    ///
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut GcPointerBase),
    ) {
        if self.los.filter(ptr as _) {
            return f(self, ptr.cast());
        }
        if self.immix.filter(Address::from_ptr(ptr)) {
            return f(self, ptr.cast());
        }
    }
}

/// Visits garbage collected objects
pub struct SlotVisitor {
    pub(super) queue: Vec<*mut GcPointerBase>,
    pub(super) cons_roots: Vec<(usize, usize)>,
    pub(super) bytes_visited: usize,
    gc: *mut RegionGC,
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
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return GcPointer {
                    base: NonNull::new_unchecked(base as *mut _),
                    marker: Default::default(),
                };
            }
            if (*self.gc).immix.filter_fast(Address::from_ptr(*cell)) {
                let block = ImmixBlock::get_block_ptr(Address::from_ptr(*cell));
                (*block).line_object_mark(Address::from_ptr(*cell));
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
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return *cell;
            }

            if (*self.gc).immix.filter_fast(Address::from_ptr(base)) {
                let block = ImmixBlock::get_block_ptr(Address::from_ptr(base));
                (*block).line_object_mark(Address::from_ptr(base));
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

impl GarbageCollector for RegionGC {
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
            if size <= 31 * 1024 {
                let pointer = self.immix.allocate(size);
                if likely(!pointer.is_null()) {
                    self.allocated += size;

                    pointer.write(GcPointerBase::new(vtable, type_id));
                    return Some(NonNull::new_unchecked(pointer));
                }
            }
            let pointer = self.los.allocate(size);

            pointer
                .cast::<GcPointerBase>()
                .write(GcPointerBase::new(vtable, type_id));

            return Some(NonNull::new_unchecked(pointer.cast::<GcPointerBase>()));
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
        if self.immix.allocated() + self.los.allocated > self.threshold {
            self.gc();
        }
    }

    fn gc(&mut self) {
        unsafe {
            self.collect_internal(&0);
        }
    }

    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        self.los.walk(callback);
        self.immix.walk(callback);
    }
}
