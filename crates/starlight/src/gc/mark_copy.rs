use std::ptr::NonNull;

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

pub struct CopyingVisitor {
    queue: Vec<*mut GcPointerBase>,
    bytes_visited: usize,
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
        let mut constraints = std::mem::replace(&mut self.constraints, vec![]);
        for _constraint in constraints.iter_mut() {
            //     constraint.execute(visitor);
        }
        std::mem::swap(&mut self.constraints, &mut constraints);
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
        let p = self.alloc.bump_alloc(align_usize(size, 16));
        unsafe {
            if p.is_null() {
                panic!("out of memory");
            }
            let base = p.to_mut_ptr::<GcPointerBase>();
            base.write(GcPointerBase::new(vtable));
            Some(NonNull::new_unchecked(base))
        }
    }

    fn gc(&mut self) {
        todo!()
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
                let sz = (*p).get_dyn().compute_size() + size_of::<GcPointerBase>();
                callback(p, sz);
                scan = scan.offset(align_usize(sz, 16));
            }
        }
    }
}
