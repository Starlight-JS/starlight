use crate::{gc::*, heap::cell::*, vm::GcParams};
use std::collections::LinkedList;
use std::mem::transmute;

/// Visits garbage collected objects
pub struct SlotVisitor {
    queue: Vec<*mut GcPointerBase>,
    cons_roots: Vec<(usize, usize)>,
    bytes_visited: usize,
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
use yastl::Pool;
pub struct MiGC {
    weak_slots: LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    threadpool: Option<Pool>,
    n_workers: u32,
    max_heap_size: usize,
    collect_conservative: bool,
    mi_heap: *mut libmimalloc_sys::mi_heap_t,
}

impl MiGC {
    pub fn new(gc_params: GcParams) -> Self {
        let this = Self {
            n_workers: gc_params.nmarkers as _,
            collect_conservative: gc_params.conservative_marking,
            threadpool: if gc_params.parallel_marking {
                Some(Pool::new(gc_params.nmarkers as _))
            } else {
                None
            },
            weak_slots: Default::default(),
            constraints: vec![],
            sp: 0,
            defers: 0,

            allocated: 0,
            max_heap_size: 100 * 1024,
            mi_heap: unsafe { libmimalloc_sys::mi_heap_new() },
        };

        this
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

            cons_roots: vec![],
            queue: Vec::with_capacity(256),
        };

        unsafe {
            if self.collect_conservative {
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
            }
            self.process_roots(&mut visitor);
            drop(registers);
            if let Some(ref mut pool) = self.threadpool {
                crate::heap::pmarking::start(&visitor.queue, self.n_workers as _, pool);
            } else {
                self.process_worklist(&mut visitor);
            }
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

            libmimalloc_sys::mi_heap_visit_blocks(
                self.mi_heap,
                true,
                Some(sweep),
                self as *mut Self as _,
            );

            if self.allocated > self.max_heap_size {
                self.max_heap_size = (self.allocated as f64 * 1.5f64) as usize;
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
                if self.collect_conservative {
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
                (*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
                (*ptr).get_dyn().trace(visitor);
            }
        }
    }
    /// Tried to find GC object for marking at `ptr`.
    ///
    ///
    /// TODO: Interior pointers. Right now this function just checks if `ptr` is a block allocated inside mimalloc heap.
    ///
    ///
    ///
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut GcPointerBase),
    ) {
        if libmimalloc_sys::mi_heap_check_owned(self.mi_heap, ptr.cast()) {
            if libmimalloc_sys::mi_heap_contains_block(self.mi_heap, ptr.cast()) {
                if (*ptr.cast::<GcPointerBase>()).is_allocated() {
                    f(self, ptr.cast());
                }
            }
        }
    }
}

impl GarbageCollector for MiGC {
    fn weak_slots(&mut self, cb: &mut dyn FnMut(*mut WeakSlot)) {
        for slot in self.weak_slots.iter() {
            cb(slot as *const _ as *mut _);
        }
    }
    fn stats(&self) -> GcStats {
        GcStats {
            allocated: self.allocated,
            threshold: self.max_heap_size,
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
    fn allocate(&mut self, size: usize, vtable: usize) -> Option<NonNull<GcPointerBase>> {
        unsafe {
            let pointer = if size <= libmimalloc_sys::MI_SMALL_SIZE_MAX {
                libmimalloc_sys::mi_heap_malloc_small(self.mi_heap, size)
            } else {
                libmimalloc_sys::mi_heap_malloc_aligned(self.mi_heap, size, 16)
            }
            .cast::<GcPointerBase>();

            pointer.write(GcPointerBase::new(vtable, size as _));
            self.allocated += size;
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
        if self.allocated > self.max_heap_size {
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
    let heap = &mut *(arg.cast::<MiGC>());
    let ptr = block.cast::<GcPointerBase>();
    if (*ptr).state() == DEFINETELY_WHITE {
        std::ptr::drop_in_place((*ptr).get_dyn());
        (*ptr).deallocate();
        libmimalloc_sys::mi_free(ptr.cast());
    } else {
        heap.allocated += block_sz;
        assert!((*ptr).set_state(POSSIBLY_BLACK, DEFINETELY_WHITE));
    }

    true
}

impl Drop for MiGC {
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
