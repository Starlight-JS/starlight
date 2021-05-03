/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! MallocGC. Very simple and dumb garbage collector that uses libc::malloc and libc::free functions.
//! Note that this GC scheme should be used *only* for debugging purposes, it is not designed to be fast or
//! to have small memory usage.

use super::*;
use std::collections::LinkedList;

/// MallocGC type. See module level documentation for more information.
pub struct MallocGC {
    weak_slots: LinkedList<WeakSlot>,
    allocations: Vec<*mut GcPointerBase>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    allocated: usize,
    threshold: usize,
    defers: usize,
}

impl MallocGC {
    /// Allocate `size` bytes using libc malloc. This function does not
    /// allocate somehow aligned memory.
    fn malloc(&mut self, size: usize) -> Address {
        unsafe {
            let memory = libc::malloc(size).cast::<GcPointerBase>();
            self.allocated += size;
            self.allocations.push(memory);
            Address::from_ptr(memory)
        }
    }

    pub fn new(_params: GcParams) -> Self {
        Self {
            weak_slots: Default::default(),
            allocations: Vec::with_capacity(256),
            allocated: 0,
            threshold: 256 * 1024,
            constraints: vec![],
            defers: 0,
        }
    }

    fn collect(&mut self) {
        if self.defers > 0 {
            return;
        }

        let mut collector = Collector {
            queue: Vec::with_capacity(64),
            gc: self,
        };
        collector.run();
        let mut allocated = self.allocated;
        self.allocations.retain(|pointer| unsafe {
            let object = &mut **pointer;
            if !object.set_state(POSSIBLY_BLACK, DEFINETELY_WHITE) {
                allocated -= (*object).get_dyn().compute_size() + 16;
                std::ptr::drop_in_place(object.get_dyn());

                libc::free(object as *mut GcPointerBase as *mut _);
                false
            } else {
                true
            }
        });
    }
}

pub struct Collector<'a> {
    gc: &'a mut MallocGC,
    queue: Vec<*mut GcPointerBase>,
}

impl<'a> Collector<'a> {
    fn run(&mut self) {
        self.process_roots();
        self.process_worklist();
        self.update_weak_references();
        self.reset_weak_references();
    }
    fn process_roots(&mut self) {
        let mut constraints = std::mem::replace(&mut self.gc.constraints, vec![]);
        for constraint in constraints.iter_mut() {
            constraint.execute(self);
        }
        std::mem::swap(&mut self.gc.constraints, &mut constraints);
    }

    fn process_worklist(&mut self) {
        while let Some(ptr) = self.queue.pop() {
            unsafe {
                assert!((*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK));
                (*ptr).get_dyn().trace(self);
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
        for slot in self.gc.weak_slots.iter_mut() {
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
                        if (*slot.value).state() == DEFINETELY_WHITE {
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
        let mut cursor = self.gc.weak_slots.cursor_front_mut();
        while let Some(item) = cursor.current() {
            if item.state == WeakState::Free {
                cursor.remove_current();
            } else {
                item.state = WeakState::Unmarked;
                cursor.move_next();
            }
        }
    }
}

impl Tracer for Collector<'_> {
    fn visit_raw(&mut self, cell: &mut *mut GcPointerBase) -> GcPointer<dyn GcCell> {
        unsafe {
            let p = *cell;
            if (*p).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                self.queue.push(p);
            }

            GcPointer {
                base: NonNull::new_unchecked(p),
                marker: Default::default(),
            }
        }
    }

    fn visit(&mut self, cell: &mut GcPointer<dyn GcCell>) -> GcPointer<dyn GcCell> {
        self.visit_raw(&mut cell.base.as_ptr())
    }

    fn visit_weak(&mut self, slot: *const WeakSlot) {
        unsafe {
            let inner = &mut *(slot as *mut WeakSlot);
            inner.state = WeakState::Mark;
        }
    }
    fn add_conservative(&mut self, _from: usize, _to: usize) {
        unreachable!()
    }
}

impl GarbageCollector for MallocGC {
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
    fn collect_if_necessary(&mut self) {
        if self.allocated > self.threshold {
            self.collect();
        }
    }

    fn gc(&mut self) {
        self.collect();
    }

    fn allocate(
        &mut self,
        size: usize,
        vtable: usize,
        type_id: TypeId,
    ) -> Option<NonNull<GcPointerBase>> {
        let ptr = self.malloc(size).to_mut_ptr::<GcPointerBase>();
        unsafe {
            ptr.write(GcPointerBase::new(vtable, type_id));

            //(*ptr).set_allocated();
            Some(NonNull::new_unchecked(ptr))
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
        self.allocations.iter().for_each(|x| unsafe {
            callback(*x, (**x).get_dyn().compute_size() + 16);
        });
    }
}

impl Drop for MallocGC {
    fn drop(&mut self) {
        for ptr in self.allocations.iter() {
            unsafe {
                std::ptr::drop_in_place((**ptr).get_dyn());
                libc::free((*ptr).cast());
            }
        }
        self.weak_slots.clear();
    }
}
