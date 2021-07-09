/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Mostly precise garbage collector.
//!
//! # Overview
//! This GC is simple mark&sweep with segregated storage for allocation, large objects are allocated directly using
//! mimalloc. To search for GC pointers on stack we use bitmap for small objects and mimalloc API for large objects.
//! Object is usually large when its allocation size exceeds 8KB (depends on size class progression option).
//!
//! ## Rooting
//! This GC does not require you to manually root any object since it is able to identify GC pointers on the stack. Thus
//! `letroot!` is not required to use anymore.
//!
#![allow(dead_code, unused_variables)]
use self::allocation::Space;
use crate::options::Options;
use crate::vm::Runtime;
use crate::{
    gc::cell::*,
    gc::snapshot::{
        deserializer::Deserializable,
        deserializer::Deserializer,
        serializer::{Serializable, SnapshotSerializer},
    },
};
use std::collections::LinkedList;
use std::intrinsics::{copy_nonoverlapping, unlikely};
use std::mem::swap;
use std::ops::Deref;
use std::{any::TypeId, cmp::Ordering, fmt, marker::PhantomData};
use std::{
    mem::size_of,
    ptr::{null_mut, NonNull},
};
use std::{u8, usize};
use yastl::Pool;

/// Like C's offsetof but you can use it with GC-able objects to get offset from GC header to field.
///
/// The magic number 0x4000 is insignificant. We use it to avoid using NULL, since
/// NULL can cause compiler problems, especially in cases of multiple inheritance
#[macro_export]
macro_rules! gc_offsetof {
    ($name : ident. $($field: ident).*) => {
        unsafe {
            let uninit = std::mem::transmute::<_,$crate::gc::cell::GcPointer<$name>>(0x4000usize);
            let fref = &uninit.$($field).*;
            let faddr = fref as *const _ as usize;
            faddr - 0x4000
        }
    };
}

/// Just like C's offsetof.
///
/// The magic number 0x4000 is insignificant. We use it to avoid using NULL, since
/// NULL can cause compiler problems, especially in cases of multiple inheritance.
#[macro_export]
macro_rules! offsetof {
    ($name : ident . $($field: ident).*) => {
        unsafe {
            let uninit = std::mem::transmute::<_,*const $name>(0x4000usize);
            let fref = &(&*uninit).$($field).*;
            let faddr = fref as *const _ as usize;
            faddr - 0x4000
        }
    }
}

#[macro_use]
pub mod cell;
pub mod snapshot;
pub const K: usize = 1024;
pub mod incremental_marking;
pub mod mem;
pub mod os;
pub mod pmarking;
pub mod safepoint;
#[macro_use]
pub mod shadowstack;
pub mod allocation;
pub mod block;
pub mod block_allocator;
pub mod constants;
pub mod large_object_space;
pub mod space_bitmap;

pub trait MarkingConstraint {
    fn name(&self) -> &str {
        "<anonymous name>"
    }
    fn execute(&mut self, marking: &mut dyn Tracer);
}

pub struct SimpleMarkingConstraint {
    name: String,
    exec: Box<dyn FnMut(&mut dyn Tracer)>,
}
impl SimpleMarkingConstraint {
    pub fn new(name: &str, exec: impl FnMut(&mut dyn Tracer) + 'static) -> Self {
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

    fn execute(&mut self, marking: &mut dyn Tracer) {
        (self.exec)(marking);
    }
}

pub const fn round_down(x: u64, n: u64) -> u64 {
    x & !n
}

pub const fn round_up(x: u64, n: u64) -> u64 {
    round_down(x + n - 1, n)
}

pub struct GcStats {
    pub allocated: usize,
    pub threshold: usize,
}

/// Trait that defines garbage collector API.
///
/// # Implementation notes
/// - GC implementation *must* not trigger GC inside [GarbageCollector::allocate](GarbageCollector::allocate) routine.
/// - [GarbageCollector::walk](GarbageCollector::walk) *must* pass valid objects to callback.
///
///
pub trait GarbageCollector {
    /// Allocate `size` bytes on GC heap and set vtable in GC object header.
    ///
    ///
    /// ***NOTE*** This function must not trigger garbage collection cycle.
    fn allocate(
        &mut self,
        size: usize,
        vtable: usize,
        type_id: TypeId,
    ) -> Option<NonNull<GcPointerBase>>;
    fn gc(&mut self);
    fn collect_if_necessary(&mut self);
    fn stats(&self) -> GcStats;
    fn add_constraint(&mut self, contraint: Box<dyn MarkingConstraint>);
    fn make_weak_slot(&mut self, base: *mut GcPointerBase) -> *mut WeakSlot;
    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool);
    fn defer(&mut self);
    fn undefer(&mut self);
    fn weak_slots(&mut self, cb: &mut dyn FnMut(*mut WeakSlot));
}

pub fn default_heap(params: &Options) -> Heap {
    Heap::new(params)
}

impl Heap {}

pub struct FreeObject {
    size: usize,
}

impl GcCell for FreeObject {
    fn compute_size(&self) -> usize {
        self.size
    }
    fn deser_pair(&self) -> (usize, usize) {
        unreachable!()
    }
}

unsafe impl Trace for FreeObject {
    fn trace(&mut self, _visitor: &mut dyn Tracer) {
        unreachable!();
    }
}
impl Serializable for FreeObject {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {
        unreachable!()
    }
}

impl Deserializable for FreeObject {
    unsafe fn deserialize(_at: *mut u8, _deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn deserialize_inplace(_deser: &mut Deserializer) -> Self {
        unreachable!()
    }
    unsafe fn dummy_read(_deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn allocate(_rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

pub unsafe fn fill_with_free(from: usize, to: usize) {
    let mut scan = from;
    while scan < to {
        let addr = scan as *mut GcPointerBase;
        //  (*addr).vtable = 0;
        //(*addr)
        //  .cell_state
        //.store(DEFINETELY_WHITE, std::sync::atomic::Ordering::Relaxed);
        scan += size_of::<GcPointerBase>();
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address(usize);

impl Address {
    #[inline(always)]
    pub fn align_page(self) -> Address {
        mem::page_align(self.to_usize()).into()
    }

    #[inline(always)]
    pub fn align_page_down(self) -> Address {
        Address(self.0 & !(os::page_size() - 1))
    }

    #[inline(always)]
    pub fn is_page_aligned(self) -> bool {
        mem::is_page_aligned(self.to_usize())
    }
    #[inline(always)]
    pub fn from(val: usize) -> Address {
        Address(val)
    }

    #[inline(always)]
    pub fn region_start(self, size: usize) -> Region {
        Region::new(self, self.offset(size))
    }

    #[inline(always)]
    pub fn offset_from(self, base: Address) -> usize {
        debug_assert!(self >= base);

        self.to_usize() - base.to_usize()
    }

    #[inline(always)]
    pub fn offset(self, offset: usize) -> Address {
        Address(self.0 + offset)
    }

    #[inline(always)]
    pub fn sub(self, offset: usize) -> Address {
        Address(self.0 - offset)
    }

    #[inline(always)]
    pub fn add_ptr(self, words: usize) -> Address {
        Address(self.0 + words * std::mem::size_of::<usize>())
    }

    #[inline(always)]
    pub fn sub_ptr(self, words: usize) -> Address {
        Address(self.0 - words * std::mem::size_of::<usize>())
    }

    #[inline(always)]
    pub fn to_usize(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub fn from_ptr<T>(ptr: *const T) -> Address {
        Address(ptr as usize)
    }

    #[inline(always)]
    pub fn to_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    #[inline(always)]
    pub fn to_mut_ptr<T>(&self) -> *mut T {
        self.0 as *const T as *mut T
    }

    #[inline(always)]
    pub fn null() -> Address {
        Address(0)
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub fn is_non_null(self) -> bool {
        self.0 != 0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:x}", self.to_usize())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:x}", self.to_usize())
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Address) -> Ordering {
        self.to_usize().cmp(&other.to_usize())
    }
}

impl From<usize> for Address {
    fn from(val: usize) -> Address {
        Address(val)
    }
}

#[derive(Copy, Clone)]
pub struct Region {
    pub start: Address,
    pub end: Address,
}

impl Region {
    pub fn new(start: Address, end: Address) -> Region {
        debug_assert!(start <= end);

        Region { start, end }
    }

    #[inline(always)]
    pub fn contains(&self, addr: Address) -> bool {
        self.start <= addr && addr < self.end
    }

    #[inline(always)]
    pub fn valid_top(&self, addr: Address) -> bool {
        self.start <= addr && addr <= self.end
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.end.to_usize() - self.start.to_usize()
    }

    #[inline(always)]
    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    #[inline(always)]
    pub fn disjunct(&self, other: &Region) -> bool {
        self.end <= other.start || self.start >= other.end
    }

    #[inline(always)]
    pub fn overlaps(&self, other: &Region) -> bool {
        !self.disjunct(other)
    }

    #[inline(always)]
    pub fn fully_contains(&self, other: &Region) -> bool {
        self.contains(other.start) && self.valid_top(other.end)
    }
}

impl Default for Region {
    fn default() -> Region {
        Region {
            start: Address::null(),
            end: Address::null(),
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

pub struct FormattedSize {
    pub size: usize,
}

impl fmt::Display for FormattedSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ksize = (self.size as f64) / 1024f64;

        if ksize < 1f64 {
            return write!(f, "{}B", self.size);
        }

        let msize = ksize / 1024f64;

        if msize < 1f64 {
            return write!(f, "{:.1}K", ksize);
        }

        let gsize = msize / 1024f64;

        if gsize < 1f64 {
            write!(f, "{:.1}M", msize)
        } else {
            write!(f, "{:.1}G", gsize)
        }
    }
}

pub fn formatted_size(size: usize) -> FormattedSize {
    FormattedSize { size }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_offsetof() {
        #[repr(C)]
        struct Point {
            x: u32,
            y: u32,
        }

        assert_eq!(offsetof!(Point.x), 0);
        assert_eq!(offsetof!(Point.y), 4);
    }
}

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
            if !(*base).is_white() {
                return GcPointer {
                    base: NonNull::new_unchecked(base as *mut _),
                    marker: Default::default(),
                };
            }
            (*base).force_set_state(POSSIBLY_GREY);
            self.heap.mark(*cell);
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
            if !(*base).is_white() {
                return *cell;
            }
            (*base).force_set_state(POSSIBLY_GREY);
            self.heap.mark(cell.base.as_ptr());
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

                #[cfg(target_pointer_width = "64")]
                {
                    // on 64 bit platforms we have nice opportunity to check if JS value on stack is
                    // object.
                    let val = core::mem::transmute::<_, crate::JsValue>(ptr);
                    if val.is_object() && !val.is_empty() {
                        let ptr = val.get_pointer();
                        if (*self.heap).is_heap_pointer(ptr.cast()) {
                            let mut ptr = ptr.cast::<GcPointerBase>();
                            self.visit_raw(&mut ptr);
                        }
                    }
                }
                scan += size_of::<usize>();
            }
        }
        //self.cons_roots.push((from, to));
    }
}
pub struct Heap {
    weak_slots: LinkedList<WeakSlot>,
    pub(super) constraints: Vec<Box<dyn MarkingConstraint>>,
    sp: usize,
    defers: usize,
    allocated: usize,
    threadpool: Option<Pool>,
    n_workers: u32,
    max_heap_size: usize,
    space: Space,
    verbose: bool,
    pub(super) progression: f64,
    allocation_color: u8,
    pub(super) current_white_part: u8,
}

impl Heap {
    pub(super) fn flip_white_part(&mut self) {
        self.current_white_part = other_white_part(self.current_white_part);
    }
    pub fn new(opts: &Options) -> Self {
        Self {
            allocation_color: DEFINETELY_WHITE,
            weak_slots: LinkedList::new(),
            constraints: vec![],
            sp: 0,
            current_white_part: DEFINETELY_WHITE,
            defers: 0,
            progression: opts.incremental_gc_progression,
            verbose: opts.verbose_gc,
            allocated: 0,
            max_heap_size: 256 * 1024,
            threadpool: if opts.parallel_marking {
                Some(Pool::new(opts.gc_threads as _))
            } else {
                None
            },
            n_workers: opts.gc_threads as _,
            space: Space::new(
                opts.heap_size,
                opts.dump_size_classes,
                opts.size_class_progression,
            ),
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
        // Capture all the registers to scan them conservatively. Note that this also captures
        // FPU registers too because JS values is NaN boxed and exist in FPU registers.

        // Get stack pointer for scanning thread stack.
        let sp: usize = 0;
        let sp = &sp as *const usize;
        self.sp = sp as usize;
        if self.defers > 0 {
            return;
        }
        logln_if!(
            unlikely(self.verbose),
            "[GC] Starting GC with {:.4} KB allocated and {:.4} KB threshold ",
            self.allocated as f64 / 1024.,
            self.max_heap_size as f64 / 1024.
        );
        let sp = self.sp;
        let mut visitor = SlotVisitor {
            bytes_visited: 0,
            queue: Vec::with_capacity(256),
            heap: unsafe { std::mem::transmute(&self.space) },
        };
        crate::vm::thread::THREAD.with(|thread| {
            visitor.add_conservative(thread.bounds.origin as _, sp as usize);
        });
        /*let regs = crate::vm::thread::Thread::capture_registers();
        visitor.add_conservative(
            regs.as_ptr() as usize,
            regs.last().unwrap() as *const _ as usize,
        );*/
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
        logln_if!(
            unlikely(self.verbose),
            "[GC] Sweep {:.4}->{:.4} KB",
            alloc as f64 / 1024.,
            self.allocated as f64 / 1024.
        );

        if self.allocated > self.max_heap_size {
            self.max_heap_size = (self.allocated as f64 * 1.5f64) as usize;
            logln_if!(
                unlikely(self.verbose),
                "[GC] New threshold: {:.4}KB",
                self.max_heap_size as f64 / 1024.
            );
        }
        logln_if!(unlikely(self.verbose), "[GC] End");
    }
    pub fn allocate_(
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
            (*ptr).force_set_state(self.allocation_color);

            Some(NonNull::new_unchecked(ptr))
        }
    }

    pub fn collect_if_necessary(&mut self) {
        if self.allocated >= self.max_heap_size {
            self.gc();
        }
    }

    pub fn gc(&mut self) {
        self.collect_internal();
    }

    pub fn stats(&self) -> crate::gc::GcStats {
        GcStats {
            allocated: self.allocated,
            threshold: self.max_heap_size,
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
    }
    pub fn weak_slots(&mut self, cb: &mut dyn FnMut(*mut WeakSlot)) {
        for slot in self.weak_slots.iter() {
            cb(slot as *const _ as *mut _);
        }
    }

    pub fn add_constraint(&mut self, constraint: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(constraint));
    }
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

    pub fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        self.space.for_each_cell(callback);
    }

    #[inline]
    pub fn allocate_raw(
        &mut self,
        vtable: *mut (),
        size: usize,
        type_id: TypeId,
    ) -> *mut GcPointerBase {
        let real_size = size + size_of::<GcPointerBase>();
        let memory = self.allocate_(real_size, vtable as _, type_id);
        memory.map(|x| x.as_ptr()).unwrap_or_else(null_mut)
    }
    #[inline]
    pub fn allocate<T: GcCell>(&mut self, value: T) -> GcPointer<T> {
        let size = value.compute_size();
        let memory = self.allocate_raw(
            vtable_of(&value) as _,
            size + size_of::<GcPointerBase>(),
            TypeId::of::<T>(),
        );
        unsafe {
            (*memory).data::<T>().write(value);
            GcPointer {
                base: NonNull::new_unchecked(memory),
                marker: PhantomData,
            }
        }
    }

    #[inline]
    pub fn copy<T: GcCell>(&mut self, value: GcPointer<T>) -> GcPointer<T> {
        let obj = value.deref();
        let size = obj.compute_size();
        let memory = self.allocate_raw(vtable_of(obj) as _, size, TypeId::of::<T>());
        unsafe {
            let base = &*(value.base.as_ptr());
            copy_nonoverlapping(base.data::<u8>(), (*memory).data::<u8>(), size);
            GcPointer {
                base: NonNull::new_unchecked(memory),
                marker: PhantomData,
            }
        }
    }

    pub fn make_null_weak<T: GcCell>(&mut self) -> WeakRef<T> {
        let slot = self.make_weak_slot(null_mut());
        unsafe {
            WeakRef {
                inner: NonNull::new_unchecked(slot),
                marker: Default::default(),
            }
        }
    }

    pub fn make_weak<T: GcCell>(&mut self, p: GcPointer<T>) -> WeakRef<T> {
        let slot = self.make_weak_slot(p.base.as_ptr());
        unsafe {
            WeakRef {
                inner: NonNull::new_unchecked(slot),
                marker: Default::default(),
            }
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        self.space.sweep();
    }
}
