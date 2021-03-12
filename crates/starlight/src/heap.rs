//! # Starlight's memory heap
//!
//!
//! In JavaScript memory is managed automatically and invisibly to users. Starlight for this "magic" implements
//! tracing garbage collector.
//!
//! # Algorithm
//!
//! - *Marking*: The collector marks objects as it finds references to them. Objects not marked are deleted.
//!  Most of the collector’s time is spent visiting objects to find references to other objects.
//! - *Constraints*: The collector allows to supply additional constraints on when objects should
//!  be marked, to support custom object lifetime rules.
//! - *Conservatism*: The collector scans the stack and registers conservatively, that is, checking each word
//!     to see if it is in the bounds of some object and then marking it if it is. This means that all of the Rust, assembly, and just-in-time (JIT) compiler-generated code in our system
//!     can store heap pointers in local variables without any hassles.
//!
//!
//! # Allocation
//! [mimalloc](https://github.com/microsoft/mimalloc) is used for allocation. MiMalloc offers great allocation speed, cache locality,
//! wide range of API that helps us to implement sweeping and conservative scanning very effectively.
//!
//! # Conservative roots
//!
//!
//! Garbage collection begins by looking at local variables and some global state to figure out the initial set
//! of marked objects. Introspecting the values of local variables is tricky. Starlight uses Rust local variables
//! for pointers to the garbage collector’s heap, but C-like languages provide no facility for precisely
//! introspecting the values of specific variables of arbitrary stack frames. Starlight solves this problem
//! by marking objects conservatively when scanning roots. We use mimalloc in part because it makes it easy to ask whether an arbitrary pointer could possibly be a pointer to some object.
//! We view this as an important optimization. Without conservative root scanning, Rust code would have to use some API to notify the collector about what objects it points to. Conservative root scanning means not having to do any of that work.
//!
//!
//!
//! # Why not reference counting?
//!
//!
//! I considered using reference counting for Starlight but then decided to keep using mostly-precise GC. RC is not just
//! slower at mutator times but also does not prevent cycles, increases object header size and makes it harder to work with
//! objects at JIT level.
//!
//!
//! # Why not use existing Rust crate for GC?
//!
//!
//! All of these crates is fully precise collectors which require you to use special API to handle rooted objects which
//! sometimes comes with big overhed (i.e rust-gc crate which is really slow). Our GC allows for GC pointers to be copyable
//! and handled with zero overhead.
//!
//! # Why not use BDWGC?
//!
//! Boehm-Demers-Wise's GC is great library and it is used in many projects but it does not suit Starlight use:
//! - It can't scan Rust standard library types.
//! - No support for weak references which is necessary for inline caches and JS `WeakRef` type.
//! - Quite slow compared to what we have implemented.
//! - There's no API for proper precise marking.
//!
#![allow(dead_code)]
use crossbeam::queue::SegQueue;
use std::{
    collections::{HashMap, VecDeque},
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
    mi_free, mi_good_size, mi_heap_area_t, mi_heap_check_owned, mi_heap_collect,
    mi_heap_contains_block, mi_heap_destroy, mi_heap_t, mi_heap_visit_blocks,
};
use wtf_rs::keep_on_stack;
pub mod cell;
pub mod snapshot;
/// Visits garbage collected objects
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
    /// Visit a reference to the specified value
    pub fn visit<T: GcCell + ?Sized>(&mut self, value: &GcPointer<T>) {
        unsafe {
            let base = value.base.as_ptr();
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return;
            }
            self.queue.push_back(base);
        }
    }
    /// Add pointer range for scanning for GC objects.
    pub fn add_conservative_roots(&mut self, from: usize, to: usize) {
        self.cons_roots.push((from, to));
    }
    /// Visit weak reference.
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

/// Garbage collected heap.
pub struct Heap {
    list: *mut GcPointerBase,
    #[allow(dead_code)]
    large: OrderedSet<*mut GcPointerBase>,
    pub(crate) weak_slots: std::collections::LinkedList<WeakSlot>,
    constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    should_stop: AtomicBool,
    max_heap_size: usize,
    track_allocations: bool,
    allocations: HashMap<*mut GcPointerBase, String>,
    pub(crate) mi_heap: *mut libmimalloc_sys::mi_heap_t,
    write_queue: SegQueue<usize>,
    gc_state: AtomicU8,
    needs_to_stop: AtomicBool,
}

impl Heap {
    pub(crate) fn walk(heap: *mut mi_heap_t, mut closure: impl FnMut(usize, usize) -> bool) {
        unsafe extern "C" fn walk(
            _heap: *const mi_heap_t,
            _area: *const mi_heap_area_t,
            block: *mut libc::c_void,
            block_sz: usize,
            arg: *mut libc::c_void,
        ) -> bool {
            if block.is_null() {
                return true;
            }
            let closure: *mut dyn FnMut(usize, usize) -> bool =
                std::mem::transmute(*arg.cast::<(usize, usize)>());

            (&mut *closure)(block as usize, block_sz)
        }

        let f: &mut dyn FnMut(usize, usize) -> bool = &mut closure;
        let trait_obj: (usize, usize) = unsafe { std::mem::transmute(f) };
        unsafe {
            mi_heap_visit_blocks(
                heap,
                true,
                Some(walk),
                &trait_obj as *const (usize, usize) as _,
            );
        }
    }
    #[deprecated(since = "0.0.0", note = "Write barrier is not used anywhere for now")]
    pub fn write_barrier<T: GcCell, U: GcCell>(&self, object: GcPointer<T>, field: GcPointer<U>) {
        unsafe {
            let object_base = &mut *object.base.as_ptr();
            let field_base = &mut *field.base.as_ptr();
            if object_base.state() == POSSIBLY_BLACK && field_base.state() == DEFINETELY_WHITE {
                self.add_to_remembered_set(object_base);
            }
        }
    }

    fn add_to_remembered_set(&self, object: &mut GcPointerBase) {
        object.force_set_state(POSSIBLY_GREY);
        self.write_queue.push(object as *mut _ as usize);
    }
    /// Create null WeakRef. This weak will always return None on [WeakRef::upgrade](WeakRef::upgrade).  
    pub fn make_null_weak<T: GcCell>(&mut self) -> WeakRef<T> {
        let slot = WeakSlot {
            value: 0 as *mut _,
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        unsafe {
            let weak = WeakRef {
                inner: NonNull::new_unchecked(self.weak_slots.back().unwrap() as *const _ as *mut _),
                marker: Default::default(),
            };
            weak
        }
    }
    /// Create WeakRef<T> from GC pointer.
    pub fn make_weak<T: GcCell>(&mut self, p: GcPointer<T>) -> WeakRef<T> {
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
            weak
        }
    }
    /// Create WeakRef<T> from GC pointer.
    pub fn make_weak_slot(&mut self, p: *mut GcPointerBase) -> *mut WeakSlot {
        let slot = WeakSlot {
            value: p,
            state: WeakState::Unmarked,
        };
        self.weak_slots.push_back(slot);
        {
            self.weak_slots.back_mut().unwrap() as *mut _
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
            max_heap_size: 100 * 1024,
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
    /// Add marking constraint to constraints list. It will be executed at start of GC cycle.
    pub fn add_constraint(&mut self, constraint: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(constraint));
    }
    /// Trigger GC cycle if necessary.
    pub fn collect_if_necessary(&mut self) {
        if self.allocated >= self.max_heap_size {
            self.gc();
        }
    }
    pub fn allocate_raw(&mut self, vtable: *mut (), size: usize) -> *mut GcPointerBase {
        let real_size = size + size_of::<GcPointerBase>();
        unsafe {
            let pointer = if real_size <= libmimalloc_sys::MI_SMALL_SIZE_MAX {
                libmimalloc_sys::mi_heap_malloc_small(self.mi_heap, real_size)
            } else {
                libmimalloc_sys::mi_heap_malloc_aligned(self.mi_heap, real_size, 16)
            }
            .cast::<GcPointerBase>();
            pointer.write(GcPointerBase::new(vtable as _));
            std::ptr::copy_nonoverlapping(&0u8, (*pointer).data(), size);
            self.allocated += mi_good_size(real_size);
            return pointer;
        }
    }
    /// Allocate `value` in GC heap.
    ///
    ///
    /// Returns value allocated on GC heap. Note that this function might trigger GC cycle but in most cases
    /// it is quite fast to call function and it is most of the time is inlined.
    ///
    ///
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
    /// Defer GC. GC will not happen until [undefer](Heap::undefer) is invoked.
    ///
    /// TODO: Mark this function as unsafe or add `DeferGC` type which undefers GC when goes out of scope.
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
    /// Trigger garbage collection cycle.
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
        // Capture all the registers to scan them conservatively. Note that this also captures
        // FPU registers too because JS values is NaN boxed and exist in FPU registers.
        let registers = crate::vm::thread::Thread::capture_registers();
        // Get stack pointer for scanning thread stack.
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
            if !registers.is_empty() {
                visitor.cons_roots.push((
                    registers.first().unwrap() as *const usize as _,
                    registers.last().unwrap() as *const usize as _,
                ));
            }
            self.process_roots(&mut visitor);
            drop(registers);
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
            mi_heap_collect(self.mi_heap, false);
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
        while let Some(ptr) = visitor.queue.pop_front() {
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
        if mi_heap_check_owned(self.mi_heap, ptr.cast()) {
            if mi_heap_contains_block(self.mi_heap, ptr.cast()) {
                f(self, ptr.cast());
            }
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
    // weird, mimalloc passes NULL pointer at first iteration.
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
        assert!((*ptr).set_state(POSSIBLY_BLACK, DEFINETELY_WHITE));
    }

    true
}

impl<T: GcCell> Copy for WeakRef<T> {}
impl<T: GcCell> Clone for WeakRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}
