#![allow(dead_code)]
use std::{
    collections::{HashMap, VecDeque},
    intrinsics::unlikely,
    mem::size_of,
    mem::transmute,
    ptr::{null_mut, NonNull},
    sync::atomic::AtomicBool,
};

use self::cell::{GcCell, GcPointer, GcPointerBase, WeakRef, WeakSlot, WeakState};
use crate::utils::ordered_set::OrderedSet;
use libmimalloc_sys::{
    mi_free, mi_heap_check_owned, mi_heap_contains_block, mi_heap_destroy, mi_heap_visit_blocks,
    mi_usable_size,
};
use wtf_rs::keep_on_stack;
pub mod cell;
pub mod marker_thread;
pub struct SlotVisitor {
    queue: VecDeque<*mut GcPointerBase>,

    bytes_visited: usize,
    sp: usize,
    cons_roots: Vec<(usize, usize)>,
}
pub fn usable_size<T: GcCell + ?Sized>(value: GcPointer<T>) -> usize {
    unsafe { libmimalloc_sys::mi_usable_size(value.base.as_ptr().cast()) }
}
impl SlotVisitor {
    unsafe fn visit_raw(&mut self, base: *mut GcPointerBase) {
        if (*base).is_marked() {
            return;
        }

        (*base).mark();
        self.bytes_visited += 1;
        self.queue.push_back(base);
    }
    pub fn visit<T: GcCell + ?Sized>(&mut self, value: &GcPointer<T>) {
        unsafe {
            let base = value.base.as_ptr();
            if (*base).is_marked() {
                return;
            }
            // let trace = Backtrace::new();
            //self.trace.push_back(format!("{:?}", trace));
            (*base).mark();
            self.bytes_visited += 1;
            // self.bytes_visited += usable_size(value);
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
    list: *mut GcPointerBase,
    #[allow(dead_code)]
    large: OrderedSet<*mut GcPointerBase>,
    weak_slots: std::collections::LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    should_stop: AtomicBool,
    max_heap_size: usize,
    track_allocations: bool,
    allocations: HashMap<*mut GcPointerBase, String>,
    mi_heap: *mut libmimalloc_sys::mi_heap_t,
}

impl Heap {
    pub fn make_weak<T: GcCell>(&mut self, p: GcPointer<T>) -> GcPointer<WeakRef<T>> {
        let slot = WeakSlot {
            value: p.base.as_ptr(),
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        unsafe {
            let weak = WeakRef {
                inner: NonNull::new_unchecked(self.weak_slots.back().unwrap() as *const _ as *mut _),
                marker: Default::default(),
            };
            self.allocate(weak)
        }
    }
    pub fn new(track_allocations: bool) -> Self {
        let mut this = Self {
            allocations: HashMap::new(),
            should_stop: AtomicBool::new(false),
            track_allocations,
            list: null_mut(),
            large: OrderedSet::new(),
            weak_slots: Default::default(),
            constraints: vec![],
            sp: 0,
            defers: 0,
            allocated: 0,
            max_heap_size: 4 * 1024,
            mi_heap: unsafe { libmimalloc_sys::mi_heap_new() },
        };

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

    pub fn collect_if_necessary(&mut self) {
        if self.allocated >= self.max_heap_size {
            self.gc();
        }
    }

    #[inline]
    pub fn allocate<T: GcCell>(&mut self, value: T) -> GcPointer<T> {
        self.collect_if_necessary();

        let real_size = value.compute_size() + size_of::<GcPointerBase>();
        unsafe {
            let pointer = libmimalloc_sys::mi_heap_malloc_aligned(self.mi_heap, real_size, 16)
                .cast::<GcPointerBase>();
            let vtable = std::mem::transmute::<_, mopa::TraitObject>(&value as &dyn GcCell).vtable;
            pointer.write(GcPointerBase::new(vtable as _));
            (*pointer).data::<T>().write(value);
            (*pointer).live();
            //(*pointer).next = self.list;
            //self.list = pointer;
            self.allocated += mi_usable_size(pointer.cast());
            //  self.pointers.push(pointer as _);
            #[cold]
            #[inline(never)]
            fn track_small(p: *mut u8, size: usize, this: &mut Heap) {
                let backtrace = backtrace::Backtrace::new();
                let fmt = format!(
                    "Small allocation of size {} at {:p}\n  backtrace: \n{:?}",
                    size, p, backtrace
                );
                this.allocations.insert(p.cast(), fmt);
            }
            if unlikely(self.track_allocations) {
                track_small(pointer.cast(), mi_usable_size(pointer.cast()), self);
            }
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
        if self.defers == 0 && self.allocated >= self.max_heap_size {
            self.collect_if_necessary();
        }
    }
    #[inline(never)]
    pub fn gc(&mut self) {
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
    pub fn allocation_track<T: GcCell + ?Sized>(&self, ptr: GcPointer<T>) -> &str {
        if let Some(info) = self.allocations.get(&ptr.base.as_ptr()) {
            info
        } else {
            "<no track>"
        }
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

            if self.track_allocations {
                #[cold]
                #[inline(never)]
                unsafe fn cleanup_allocations(this: &mut Heap) {
                    this.allocations.retain(|alloc, info| {
                        if !(**alloc).is_marked() {
                            println!("retain {:p} \n{}", *alloc, info);
                        }
                        (**alloc).is_marked()
                    });
                }
                cleanup_allocations(self);
            }

            /*for arena in self.arenas.iter() {
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
            });*/
            //let mut allocated = 0;
            /*self.pointers.retain(|pointer| {
                let ptr = *pointer as *mut GcPointerBase;
                if (*ptr).is_marked() {
                    (*ptr).unmark();

                    allocated += mi_usable_size(ptr.cast());
                    true
                } else {
                    (*ptr).dead();
                    std::ptr::drop_in_place((*ptr).get_dyn());
                    mi_free(ptr.cast());
                    false
                }
            });*/
            //let mut prev = null_mut();
            //let mut cur = self.list;
            self.allocated = 0;
            //self.list = null_mut();
            /* while !cur.is_null() {
                let sz = mi_usable_size(cur.cast());
                if (*cur).is_marked() {
                    prev = cur;
                    cur = (*cur).next;
                    (*prev).unmark();

                    self.allocated += sz;
                } else {
                    let unreached = cur;
                    cur = (*cur).next;
                    if !prev.is_null() {
                        (*prev).next = cur;
                    } else {
                        self.list = cur;
                    }
                    (*unreached).dead();
                    std::ptr::drop_in_place((*unreached).get_dyn());
                    mi_free(unreached.cast());
                }
            }*/
            //self.pointers.shrink_to_fit();
            /*self.pointers = OrderedSet::from_sorted_set(Vec::with_capacity(visitor.bytes_visited));*/
            libmimalloc_sys::mi_heap_visit_blocks(
                self.mi_heap,
                true,
                Some(sweep),
                self as *mut Self as _,
            );
            //   self.allocated = allocated;
            // self.allocated = visitor.bytes_visited;
            if self.allocated > self.max_heap_size {
                self.max_heap_size = (self.allocated as f64 * 1.6f64) as usize;
            }
        }
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
    #[doc(hidden)]
    pub fn process_roots(&mut self, visitor: &mut SlotVisitor) {
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
                    self.find_gc_object_pointer_for_marking(ptr, |_, ptr| {
                        visitor.visit_raw(ptr);
                        found = true;
                    });
                    if !found {
                        let value = transmute::<_, crate::vm::value::JsValue>(ptr);
                        if value.is_object() {
                            self.find_gc_object_pointer_for_marking(
                                value.get_pointer().cast(),
                                |_, ptr| {
                                    visitor.visit_raw(ptr);
                                },
                            );
                        }
                    }
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
        /*if !self.large.is_empty() {
            if (**self.large.first().unwrap()).above_lower_bound(ptr.cast())
                && (**self.large.last().unwrap()).below_upper_bound(ptr.cast())
            {
                let result = self
                    .large
                    .binary_search(&(PreciseAllocation::from_cell(ptr.cast())));
                match result {
                    Ok(index) => {
                        f(self, (*self.large[index]).cell());
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
            let live = (*candidate).header().is_live(p.cast());
            if live && (*p).is_live() {
                f(self, p.cast());
            }
            live
        };

        if try_ptr(ptr.cast()) {
            return;
        }
        let aligned = (*candidate).header().cell_align(ptr.cast());
        try_ptr(aligned as *mut _);*/
        /*
        if !self.pointers.is_empty() {
            if self.pointers.contains(&(ptr as usize)) {
                f(self, ptr.cast());
            }
        }*/

        //if mi_is_in_heap_region(ptr.cast()) {
        if mi_heap_check_owned(self.mi_heap, ptr.cast()) {
            if mi_heap_contains_block(self.mi_heap, ptr.cast()) {
                if (*ptr.cast::<GcPointerBase>()).is_live() {
                    f(self, ptr.cast());
                }
            }
        }
        //}
    }

    pub fn defer_gc(&mut self) {
        self.defers += 1;
    }

    pub fn undefer_gc(&mut self) {
        self.defers -= 1;
        if self.defers == 0 {
            self.collect_if_necessary();
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            /*let mut cur = self.list;
            while !cur.is_null() {
                let next = (*cur).next;
                std::ptr::drop_in_place((*cur).get_dyn());
                cur = next;
            }*/
            mi_heap_visit_blocks(self.mi_heap, true, Some(sweep), self as *mut Self as _);
            mi_heap_destroy(self.mi_heap);
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
    if block.is_null() {
        return true;
    }
    let heap = &mut *(arg.cast::<Heap>());
    let ptr = block.cast::<GcPointerBase>();
    if !(*ptr).is_marked() {
        (*ptr).dead();
        (*ptr).unmark();
        std::ptr::drop_in_place((*ptr).get_dyn());

        mi_free(ptr.cast());
    } else {
        heap.allocated += block_sz;
        //(*ptr).next = heap.list;
        //heap.list = ptr;
        (*ptr).unmark();
    }

    true
}

extern "C" {
    fn mi_is_in_heap_region(p: *const u8) -> bool;
}
