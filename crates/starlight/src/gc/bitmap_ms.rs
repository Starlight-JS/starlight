use super::{accounting::space_bitmap::SpaceBitmap, freelist::FreeList, *};
use cell::*;
use mem::align_usize;
use memmap2::MmapMut;
use std::collections::LinkedList;
use swc_common::Mark;
pub struct MarkAndSweep {
    freelist: FreeList,
    weak_slots: LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    allocated: usize,
    threshold: usize,
    mark_bitmap: SpaceBitmap<8>,
    live_bitmap: SpaceBitmap<8>,
    mmap: MmapMut,
    defers: usize,
    heap_begin: usize,
    heap_size: usize,
}

impl MarkAndSweep {
    fn malloc(&mut self, size: usize) -> Address {
        let size = align_usize(size, 8);
        self.allocated += size;
        let p = self.freelist.alloc_and_coalesce(size);
        assert!(!p.is_null());
        self.live_bitmap.set(p.to_usize());

        p
    }

    pub fn new(params: GcParams) -> Self {
        let mut map =
            MmapMut::map_anon(params.heap_size).expect("Failed to create Mark&Sweep space");
        let heap_begin = align_usize(map.as_mut_ptr() as usize, 8);
        let mut freelist = FreeList::new();

        let sz = params.heap_size - (heap_begin - map.as_mut_ptr() as usize);
        freelist.add(Address::from(heap_begin), sz);
        Self {
            heap_begin,
            weak_slots: Default::default(),
            heap_size: sz,
            live_bitmap: SpaceBitmap::create("M&S bitmap #1", heap_begin as _, sz),
            mark_bitmap: SpaceBitmap::create("M&S bitmap #2", heap_begin as _, sz),
            mmap: map,
            freelist,
            allocated: 0,
            threshold: 100 * 1024,
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

        let sweep_begin = self.heap_begin;
        let sweep_end = sweep_begin + self.heap_size;

        /*unsafe {
            let mut allocated = self.allocated;
            SpaceBitmap::<8>::sweep_walk(
                &self.live_bitmap,
                &self.mark_bitmap,
                sweep_begin,
                sweep_end,
                |count, objects| {
                    let slice =
                        std::slice::from_raw_parts(objects as *const *mut GcPointerBase, count);

                    for ptr in slice {
                        let p = (*ptr) as usize;
                        if !ptr.is_null()
                            && (p >= sweep_begin && p < sweep_end)
                            && self.live_bitmap.test((*ptr) as usize)
                        {
                            if (**ptr).is_allocated() {
                                (**ptr).deallocate();
                                println!("Free {:p}", *ptr);
                                core::ptr::drop_in_place((**ptr).get_dyn());
                                let sz = (**ptr).size as usize;
                                freelist.add(Address::from_ptr(*ptr), sz);
                                allocated -= sz;
                            }
                        }
                    }
                },
            );
            self.allocated = allocated;
            self.freelist = freelist;
            if self.allocated >= self.threshold {
                self.threshold = (self.allocated as f64 * 1.5) as usize;
                if self.threshold > self.heap_size {
                    self.threshold = self.heap_size;
                }
            }
        }*/
        unsafe {
            let mut allocated = self.allocated;
            let mut freelist = std::mem::replace(&mut self.freelist, FreeList::new());
            let live: &mut SpaceBitmap<8> = &mut *(&mut self.live_bitmap as *mut _);
            self.live_bitmap
                .visit_marked_range(sweep_begin, sweep_end, |object| {
                    let object = object as *mut GcPointerBase;
                    if (*object).state() == DEFINETELY_WHITE {
                        live.clear(object as usize);
                        core::ptr::drop_in_place((*object).get_dyn());
                        freelist.add(Address::from_ptr(object), (*object).size as _);
                    } else {
                        assert!((*object).set_state(POSSIBLY_BLACK, DEFINETELY_WHITE));
                    }
                });

            self.allocated = allocated;
            self.freelist = freelist;
            if self.allocated >= self.threshold {
                self.threshold = (self.allocated as f64 * 1.5) as usize;
                if self.threshold > self.heap_size {
                    self.threshold = self.heap_size;
                }
            }
        }
        std::mem::swap(&mut self.mark_bitmap, &mut self.live_bitmap);
        self.mark_bitmap.clear_to_zeros();
    }
}
impl GarbageCollector for MarkAndSweep {
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

    fn allocate(&mut self, size: usize, vtable: usize) -> Option<NonNull<GcPointerBase>> {
        let size = align_usize(size, 8);
        let ptr = self.malloc(size).to_mut_ptr::<GcPointerBase>();
        unsafe {
            ptr.write(GcPointerBase::new(vtable, size as _));
            (*ptr)
                .cell_state
                .store(DEFINETELY_WHITE, atomic::Ordering::Relaxed);
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
        let begin = self.mmap.as_mut_ptr() as usize;
        let end = begin + self.heap_size;
        self.live_bitmap
            .visit_marked_range(begin, end - 1, |object| unsafe {
                let obj = object as *mut GcPointerBase;
                callback(obj, (*obj).size as usize);
            });
    }
}
pub struct Collector<'a> {
    queue: Vec<*mut GcPointerBase>,
    gc: &'a mut MarkAndSweep,
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
                (*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
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
