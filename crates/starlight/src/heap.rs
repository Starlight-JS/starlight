pub mod allocation;
pub mod block;
pub mod block_allocator;
pub mod collector;
pub mod constants;
pub mod large_object_space;
pub mod space_bitmap;
use yastl::Pool;

use crate::gc::{cell::*, GarbageCollector, GcStats, MarkingConstraint};
use crate::prelude::Options;
use std::collections::LinkedList;
use std::ptr::{drop_in_place, null_mut};
use std::{
    mem::{size_of, swap},
    ptr::NonNull,
};

use self::allocation::Space;
/// Visits garbage collected objects
pub struct SlotVisitor {
    pub(super) queue: Vec<*mut GcPointerBase>,
    pub(super) bytes_visited: usize,
    pub(super) heap: &'static Space,
}
unsafe impl Send for SlotVisitor {}
unsafe impl Send for Space {}
unsafe impl Sync for Space {}
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

            self.queue.push(base);
            *cell
        }
    }

    fn add_conservative(&mut self, from: usize, to: usize) {
        let mut scan = from;
        let mut end = to;
        if scan > end {
            swap(&mut scan, &mut end);
        }
        unsafe {
            while scan < end {
                let ptr = (scan as *mut *mut u8).read();

                if (*self.heap).is_heap_pointer(ptr) {
                    let mut ptr = ptr.cast::<GcPointerBase>();
                    self.visit_raw(&mut ptr);
                    scan += size_of::<usize>();
                    continue;
                }

                /*#[cfg(target_pointer_width = "64")]
                {
                    // on 64 bit platforms we have nice opportunity to check if JS value on stack is
                    // object.
                    let val = transmute::<_, JsValue>(ptr);
                    if val.is_pointer() && !val.is_empty() {
                        let ptr = val.get_pointer();
                        if (*self.heap).is_heap_pointer(ptr.cast()) {
                            let mut ptr = ptr.cast::<GcPointerBase>();
                            self.visit_raw(&mut ptr);
                        }
                    }
                }*/
                scan += size_of::<usize>();
            }
        }
        //self.cons_roots.push((from, to));
    }
}
pub struct Heap {
    weak_slots: LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    threadpool: Option<Pool>,
    n_workers: u32,
    max_heap_size: usize,
    space: Space,
}

impl Heap {
    pub fn new(progression: f64, dump: bool, size: usize, opts: &Options) -> Self {
        Self {
            weak_slots: LinkedList::new(),
            constraints: vec![],
            sp: 0,
            defers: 0,
            allocated: 0,
            max_heap_size: 128 * 1024,
            threadpool: if opts.parallel_marking {
                Some(Pool::new(opts.gc_threads as _))
            } else {
                None
            },
            n_workers: opts.gc_threads as _,
            space: Space::new(size, dump, progression),
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

    /// This function marks all potential roots. This simply means it executes
    /// all the constraints supplied to GC.

    fn process_roots(&mut self, visitor: &mut SlotVisitor) {
        unsafe {
            let mut constraints = std::mem::take(&mut self.constraints);
            for constraint in constraints.iter_mut() {
                constraint.execute(visitor);
            }
            std::mem::swap(&mut self.constraints, &mut constraints);
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

    fn collect_internal(&mut self) {
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
            queue: Vec::with_capacity(256),
            heap: unsafe { std::mem::transmute(&self.space) },
        };
        crate::vm::thread::THREAD.with(|thread| {
            visitor.add_conservative(thread.bounds.origin as _, sp as usize);
        });
        self.process_roots(&mut visitor);

        if let Some(ref mut pool) = self.threadpool {
            crate::gc::pmarking::start(
                &visitor.queue,
                unsafe { std::mem::transmute(&self.space) },
                self.n_workers as _,
                pool,
            );
        } else {
            self.process_worklist(&mut visitor);
        }
        self.update_weak_references();
        self.reset_weak_references();
        let alloc = self.allocated;
        self.allocated = self.space.sweep();

        if self.allocated > self.max_heap_size {
            self.max_heap_size = (self.allocated as f64 * 1.5f64) as usize;
        }
    }
}

impl GarbageCollector for Heap {
    fn allocate(
        &mut self,
        size: usize,
        vtable: usize,
        type_id: std::any::TypeId,
    ) -> Option<NonNull<GcPointerBase>> {
        let mut th = self.allocated;
        let ptr = self
            .space
            .allocate(size + 16, &mut th)
            .cast::<GcPointerBase>();
        self.allocated = th;
        unsafe {
            ptr.write(GcPointerBase::new(vtable, type_id));
            (*ptr).force_set_state(DEFINETELY_WHITE);

            Some(NonNull::new_unchecked(ptr))
        }
    }

    fn collect_if_necessary(&mut self) {
        if self.allocated >= self.max_heap_size {
            self.gc();
        }
    }

    fn gc(&mut self) {
        self.collect_internal();
    }

    fn stats(&self) -> crate::gc::GcStats {
        GcStats {
            allocated: self.allocated,
            threshold: self.max_heap_size,
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
    fn weak_slots(&mut self, cb: &mut dyn FnMut(*mut WeakSlot)) {
        for slot in self.weak_slots.iter() {
            cb(slot as *const _ as *mut _);
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

    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        self.space.for_each_cell(callback);
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        self.space.sweep();
    }
}
