pub use comet as cometgc;
use comet::gcref::UntypedGcRef;
use comet::header::HeapObjectHeader;
use comet::heap::{DeferPoint, Heap as CometHeap, MarkingConstraint};
pub use comet::internal::finalize_trait::FinalizeTrait as Finalize;
use comet::internal::gc_info::{GCInfoIndex, GCInfoTrait};
pub use comet::internal::trace_trait::TraceTrait as Trace;
pub use comet::visitor::Visitor;
use cometgc::gcref::GcRef;
use mopa::mopafy;
use std::collections::HashMap;
use std::intrinsics::{size_of, transmute};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::options::Options;
pub struct Heap {
    heap: Box<CometHeap>,
}
#[allow(dead_code)]
pub struct SimpleMarkingConstraint {
    name: String,
    exec: Box<dyn FnMut(&mut Visitor)>,
}
impl SimpleMarkingConstraint {
    pub fn new(name: &str, exec: impl FnMut(&mut Visitor) + 'static) -> Self {
        Self {
            name: name.to_owned(),
            exec: Box::new(exec),
        }
    }
}

impl MarkingConstraint for SimpleMarkingConstraint {
    fn execute(&mut self, vis: &mut Visitor) {
        (self.exec)(vis);
    }
}

impl Heap {
    pub fn defer(&self) -> DeferPoint {
        DeferPoint::new(&self.heap)
    }
    pub fn new(opts: &Options) -> Self {
        let mut configs = comet::Config::default();
        configs.heap_size = opts.heap_size;

        configs.size_class_progression = opts.size_class_progression;
        configs.verbose = opts.verbose_gc;

        let mut heap = CometHeap::new(configs);
        heap.add_core_constraints();
        Self { heap }
    }
    pub fn gc(&mut self) {
        self.heap.collect_garbage();
    }
    pub fn allocate_(
        &mut self,
        size: usize,
        vtable: usize,
        idx: GCInfoIndex,
    ) -> Option<NonNull<GcPointerBase>> {
        unsafe {
            let ptr = self
                .heap
                .allocate_raw(size + size_of::<GcPointerBase>(), idx);
            match ptr {
                Some(ptr) => {
                    let raw = HeapObjectHeader::from_object(ptr.get()).cast::<GcPointerBase>();
                    idx.get_mut().vtable = vtable;

                    Some(NonNull::new_unchecked(raw))
                }
                _ => None,
            }
        }
    }

    pub fn allocate_raw(
        &mut self,
        size: usize,
        vtable: usize,
        idx: GCInfoIndex,
    ) -> *mut GcPointerBase {
        self.allocate_(size, vtable, idx)
            .unwrap_or_else(|| memory_oom())
            .as_ptr()
    }

    pub fn allocate<T: GcCell + GCInfoTrait<T> + Trace + Finalize<T>>(
        &mut self,
        value: T,
    ) -> GcPointer<T> {
        let size = value.compute_size();
        let memory = self.allocate_raw(size, vtable_of(&value), T::index());
        unsafe {
            (*memory).data::<T>().write(value);
            GcPointer {
                base: NonNull::new_unchecked(memory),
                marker: PhantomData,
            }
        }
    }
    /*
    pub fn walk(&mut self,callback: &mut dyn FnMut(*mut GcPointerBase)) -> SafepointScope
    {
        let mut point = SafepointScope::new(&mut self.main);
        self.heap.for_each_cell(&point, callback, weak_refs)
    }*/
    pub fn add_constraint(&mut self, constraint: impl MarkingConstraint + 'static) {
        self.heap.add_constraint(constraint);
    }
    pub fn make_weak<T: GcCell>(&mut self, target: GcPointer<T>) -> WeakRef<T> {
        let weak = unsafe { self.heap.allocate_weak(std::mem::transmute(target)) };
        WeakRef {
            ref_: weak,
            marker: PhantomData,
        }
    }

    pub fn collect_if_necessary(&mut self) {
        self.heap.collect_if_necessary_or_defer();
    }
}

/// `GcCell` is a type that can be allocated in GC gc and passed to JavaScript environment.
///
///
/// All cells that is not part of `src/vm` treatened as dummy objects and property accesses
/// is no-op on them.
///
pub trait GcCell: mopa::Any {
    /// Used when object has dynamic size i.e arrays
    fn compute_size(&self) -> usize {
        std::mem::size_of_val(self)
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn deser_pair(&self) -> (usize, usize) {
        (0, 0)
    }
}

mopafy!(GcCell);
#[repr(transparent)]
pub struct GcPointer<T: GcCell + ?Sized> {
    base: NonNull<GcPointerBase>,
    marker: PhantomData<T>,
}

#[repr(C)]
pub struct GcPointerBase {
    hdr: HeapObjectHeader,
}

impl<T: GcCell + ?Sized> GcPointer<T> {
    pub fn untyped(self) -> UntypedGcRef {
        unsafe { std::mem::transmute(self) }
    }
    pub fn ptr_eq<U: GcCell + ?Sized>(this: &Self, other: &GcPointer<U>) -> bool {
        this.base == other.base
    }
    #[inline]
    pub fn as_dyn(self) -> GcPointer<dyn GcCell> {
        GcPointer {
            base: self.base,
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn is<U: Trace + Finalize<U> + GcCell + GCInfoTrait<U>>(self) -> bool {
        unsafe { (*self.base.as_ptr()).hdr.get_gc_info_index() == U::index() }
    }

    #[inline]
    pub fn get_dyn(&self) -> &dyn GcCell {
        unsafe { (*self.base.as_ptr()).get_dyn() }
    }

    #[inline]
    pub fn get_dyn_mut(&mut self) -> &mut dyn GcCell {
        unsafe { (*self.base.as_ptr()).get_dyn() }
    }

    #[inline]
    pub unsafe fn downcast_unchecked<U: GcCell>(self) -> GcPointer<U> {
        GcPointer {
            base: self.base,
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn downcast<U: Trace + Finalize<U> + GcCell + GCInfoTrait<U>>(
        self,
    ) -> Option<GcPointer<U>> {
        if !self.is::<U>() {
            None
        } else {
            Some(unsafe { self.downcast_unchecked() })
        }
    }
}

impl GcPointerBase {
    pub fn allocation_size(&self) -> usize {
        unsafe { comet::gc_size(&self.hdr) }
    }

    pub fn get_dyn(&self) -> &mut dyn GcCell {
        unsafe {
            std::mem::transmute(mopa::TraitObject {
                vtable: self.hdr.get_gc_info_index().get().vtable as _,
                data: self.data::<u8>() as _,
            })
        }
    }

    pub fn data<T>(&self) -> *mut T {
        unsafe {
            (self as *const Self as *mut u8)
                .add(size_of::<Self>())
                .cast()
        }
    }
}
pub fn vtable_of<T: GcCell>(x: *const T) -> usize {
    unsafe { core::mem::transmute::<_, mopa::TraitObject>(x as *const dyn GcCell).vtable as _ }
}

pub fn vtable_of_type<T: GcCell + Sized>() -> usize {
    vtable_of(core::ptr::null::<T>())
}

impl<T: GcCell + ?Sized> Copy for GcPointer<T> {}
impl<T: GcCell + ?Sized> Clone for GcPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GcCell> Deref for GcPointer<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(&*self.base.as_ptr()).data::<T>() }
    }
}
impl<T: GcCell> DerefMut for GcPointer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(&*self.base.as_ptr()).data::<T>() }
    }
}

impl<T: GcCell + ?Sized> std::fmt::Pointer for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:p}", self.base)
    }
}

impl<T: GcCell + std::fmt::Debug> std::fmt::Debug for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", **self)
    }
}
impl<T: GcCell + std::fmt::Display> std::fmt::Display for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", **self)
    }
}

pub struct WeakRef<T: GcCell> {
    ref_: comet::gcref::WeakGcRef,
    marker: PhantomData<T>,
}

impl<T: GcCell> WeakRef<T> {
    pub fn upgrade(&self) -> Option<GcPointer<T>> {
        match self.ref_.upgrade() {
            Some(ptr) => Some(GcPointer {
                base: unsafe { transmute(ptr) },
                marker: PhantomData,
            }),
            _ => None,
        }
    }
}
#[cold]
fn memory_oom() -> ! {
    eprintln!("Starlight: No memory left");
    std::process::abort();
}

pub mod cell {
    pub use super::*;
}

macro_rules! impl_prim {
    ($($t: ty)*) => {
        $(
            impl GcCell for $t {}
        )*
    };
}

impl_prim!(String bool f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 std::fs::File u128 i128);

impl<K: GcCell, V: GcCell> GcCell for HashMap<K, V> {}
impl<T: GcCell> GcCell for WeakRef<T> {}
impl<T: GcCell> GcCell for Option<T> {}
impl<T: GcCell> GcCell for Vec<T> {}

impl<T: GcCell + Trace> Trace for GcPointer<T> {
    fn trace(&self, vis: &mut Visitor) {
        unsafe {
            vis.trace_gcref(transmute::<_, GcRef<T>>(*self));
        }
    }
}
impl<T: GcCell> PartialEq for GcPointer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl<T: GcCell> Eq for GcPointer<T> {}
impl<T: GcCell> Copy for WeakRef<T> {}
impl<T: GcCell> Clone for WeakRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: GcCell> Trace for WeakRef<T> {
    fn trace(&self, vis: &mut Visitor) {
        vis.trace_gcref(self.ref_.slot())
    }
}

impl<T: GcCell> GcCell for GcPointer<T> {}
