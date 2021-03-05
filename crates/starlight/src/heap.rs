#![allow(dead_code)]
use crossbeam::queue::SegQueue;
use std::{
    collections::{HashMap, VecDeque},
    intrinsics::unlikely,
    mem::size_of,
    mem::transmute,
    ptr::{null_mut, NonNull},
    sync::atomic::{AtomicBool, AtomicU8},
};

use self::cell::{
    GcCell, GcPointer, GcPointerBase, WeakRef, WeakSlot, WeakState, DEFINETELY_WHITE,
    POSSIBLY_BLACK, POSSIBLY_GREY,
};
use crate::utils::ordered_set::OrderedSet;
use libmimalloc_sys::{
    mi_free, mi_good_size, mi_heap_check_owned, mi_heap_collect, mi_heap_contains_block,
    mi_heap_destroy, mi_heap_malloc_small, mi_heap_visit_blocks, mi_usable_size,
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
        if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
            return;
        }
        self.queue.push_back(base);
    }
    pub fn visit<T: GcCell + ?Sized>(&mut self, value: &GcPointer<T>) {
        unsafe {
            let base = value.base.as_ptr();
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return;
            }
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

pub const GC_NONE: u8 = 0;
pub const GC_PRE_MARK: u8 = 1;
pub const GC_CONCURRENT_MARK: u8 = 2;
pub const GC_AFTER_MARK: u8 = 3;
pub const GC_SWEEP: u8 = 4;

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
    write_queue: SegQueue<usize>,
    gc_state: AtomicU8,
    needs_to_stop: AtomicBool,
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
            gc_state: AtomicU8::new(0),
            list: null_mut(),
            large: OrderedSet::new(),
            weak_slots: Default::default(),
            constraints: vec![],
            sp: 0,
            defers: 0,
            write_queue: SegQueue::new(),
            allocated: 0,
            max_heap_size: 4 * 1024,
            mi_heap: unsafe { libmimalloc_sys::mi_heap_new() },
            needs_to_stop: AtomicBool::new(false),
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
            let pointer = if real_size <= libmimalloc_sys::MI_SMALL_SIZE_MAX {
                libmimalloc_sys::mi_heap_malloc_small(self.mi_heap, real_size)
            } else {
                libmimalloc_sys::mi_heap_malloc_aligned(self.mi_heap, real_size, 16)
            }
            .cast::<GcPointerBase>();
            let vtable = std::mem::transmute::<_, mopa::TraitObject>(&value as &dyn GcCell).vtable;
            pointer.write(GcPointerBase::new(vtable as _));
            (*pointer).data::<T>().write(value);
            // (*pointer).live();

            self.allocated += mi_good_size(real_size);
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
            #[cfg(feature = "enable-gc-tracking")]
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
            mi_heap_collect(self.mi_heap, true);
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

                        if cell.state() == DEFINETELY_WHITE {
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
                slot.state = WeakState::Unmarked;
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
                (*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
                (*ptr).get_dyn().trace(visitor);
            }
        }
    }
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut GcPointerBase),
    ) {
        //if mi_is_in_heap_region(ptr.cast()) {
        if mi_heap_check_owned(self.mi_heap, ptr.cast()) {
            if mi_heap_contains_block(self.mi_heap, ptr.cast()) {
                f(self, ptr.cast());
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
    if (*ptr).state() == DEFINETELY_WHITE {
        std::ptr::drop_in_place((*ptr).get_dyn());

        mi_free(ptr.cast());
    } else {
        heap.allocated += block_sz;
        //(*ptr).next = heap.list;
        //heap.list = ptr;
        assert!((*ptr).set_state(POSSIBLY_BLACK, DEFINETELY_WHITE));
    }

    true
}

extern "C" {
    fn mi_is_in_heap_region(p: *const u8) -> bool;
}
