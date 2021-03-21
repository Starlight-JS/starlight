use std::{intrinsics::unlikely, marker::PhantomData, ptr::NonNull};

use super::{bump::BumpAllocator, mem::align_usize, *};

use crate::heap::cell::*;
use crate::heap::MarkingConstraint;
pub struct MarkCopyGC {
    weak_slots: std::collections::LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    defers: usize,
    allocated: usize,
    threshold: usize,
    n_workers: u32,
    max_heap_size: usize,

    separator: Address,
    total: Region,
    alloc: BumpAllocator,
}

pub struct CopyingVisitor<'a> {
    queue: Vec<*mut GcPointerBase>,
    bytes_visited: usize,
    from_space: Region,
    heap: &'a mut MarkCopyGC,
    top: &'a mut Address,
}

impl<'a> Tracer for CopyingVisitor<'a> {
    fn add_conservative(&mut self, _from: usize, _to: usize) {}

    fn visit(&mut self, cell: &mut GcPointer<dyn GcCell>) -> GcPointer<dyn GcCell> {
        self.visit_raw(&mut unsafe { std::mem::transmute(cell.base) })
    }

    fn visit_raw(&mut self, cell: &mut *mut GcPointerBase) -> GcPointer<dyn GcCell> {
        if self.from_space.contains(Address::from_ptr(*cell)) {
            let new_addr = self.heap.copy(Address::from_ptr(*cell), self.top);

            unsafe {
                *cell = new_addr.to_mut_ptr();
                GcPointer {
                    base: NonNull::new_unchecked(new_addr.to_mut_ptr()),
                    marker: PhantomData::default(),
                }
            }
        } else {
            unsafe {
                GcPointer {
                    base: NonNull::new_unchecked(*cell),
                    marker: PhantomData::default(),
                }
            }
        }
    }
    fn visit_weak(&mut self, at: *const WeakSlot) {
        unsafe {
            let at = at as *mut WeakSlot;
            (*at).state = WeakState::Mark;
        }
    }
}

impl MarkCopyGC {
    fn copy(&self, obj_addr: Address, top: &mut Address) -> Address {
        unsafe {
            let obj = &mut *obj_addr.to_mut_ptr::<GcPointerBase>();

            if obj.is_allocated() {
                return Address::from(obj.vtable());
            }

            let addr = *top;
            let obj_size = obj.get_dyn().compute_size() + size_of::<GcPointerBase>();

            std::ptr::copy_nonoverlapping(
                obj as *mut _ as *const _ as *const u8,
                top.to_mut_ptr::<u8>(),
                obj_size,
            );

            *top = top.offset(obj_size);

            addr
        }
    }
    pub fn from_space(&self) -> Region {
        if self.alloc.limit() == self.separator {
            Region::new(self.total.start, self.separator)
        } else {
            Region::new(self.separator, self.total.end)
        }
    }

    pub fn allocated_region(&self) -> Region {
        Region::new(self.alloc.start(), self.alloc.top())
    }

    pub fn to_space(&self) -> Region {
        if self.alloc.limit() == self.separator {
            Region::new(self.separator, self.total.end)
        } else {
            Region::new(self.total.start, self.separator)
        }
    }

    unsafe fn collect_internal(&mut self) {
        // empty to-space
        let to_space = self.to_space();
        let from_space = self.from_space();

        // determine size of heap before collection
        let old_size = self.alloc.top().offset_from(from_space.start);
        let allocated = self.allocated_region();
        let mut top = to_space.start;
        let mut scan = top;
        let mut visitor = CopyingVisitor {
            top: &mut top,
            heap: self,
            bytes_visited: 0,
            queue: vec![],
            from_space,
        };
        let mut constraints = std::mem::replace(&mut visitor.heap.constraints, vec![]);
        for constraint in constraints.iter_mut() {
            constraint.execute(&mut visitor);
        }
        std::mem::swap(&mut visitor.heap.constraints, &mut constraints);

        while scan < *visitor.top {
            let object: &mut GcPointerBase = &mut *scan.to_mut_ptr();
            object.get_dyn().trace(&mut visitor);
            scan = scan.offset(object.size as _);
        }
        scan = allocated.start;
        while scan < allocated.end {
            let object: &mut GcPointerBase = &mut *scan.to_mut_ptr();
            let sz = object.size;
            if !object.is_allocated() {
                self.allocated -= sz as usize;
                std::ptr::drop_in_place(object);
            }
            scan = scan.offset(sz as _);
        }

        os::protect(
            from_space.start,
            from_space.size(),
            os::MemoryPermission::None,
        );
        self.alloc.reset(top, to_space.end);
    }
}

impl GarbageCollector for MarkCopyGC {
    fn add_constraint(&mut self, constraint: Box<dyn MarkingConstraint>) {
        self.constraints.push(constraint);
    }

    fn stats(&self) -> GcStats {
        todo!()
    }

    fn allocate(&mut self, size: usize, vtable: usize) -> Option<NonNull<GcPointerBase>> {
        let p = self.alloc.bump_alloc(size);
        self.allocated += size;
        // max object size is 4GB
        if unlikely(size > u32::MAX as usize) {
            panic!("Maximum allocation size exceeded");
        }
        unsafe {
            if p.is_null() {
                panic!("out of memory");
            }
            let base = p.to_mut_ptr::<GcPointerBase>();
            base.write(GcPointerBase::new(vtable, size as _));
            Some(NonNull::new_unchecked(base))
        }
    }

    fn gc(&mut self) {
        unsafe {
            self.collect_internal();
        }
    }

    fn collect_if_necessary(&mut self) {
        if self.allocated > self.threshold {
            self.gc();
        }
    }

    fn make_weak_slot(&mut self, base: *mut GcPointerBase) -> *mut WeakSlot {
        let slot = WeakSlot {
            value: base,
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        {
            self.weak_slots.back_mut().unwrap() as *mut _
        }
    }

    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize)) {
        let space = self.allocated_region();
        let mut top = space.start;
        let mut scan = top;

        while scan < top {
            let addr = scan.to_usize();
            let p = addr as *mut GcPointerBase;
            unsafe {
                let sz = (*p).size;
                callback(p, sz as _);
                scan = scan.offset(sz as usize);
            }
        }
    }
}
