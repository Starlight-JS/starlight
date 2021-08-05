use std::intrinsics::{size_of, transmute};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use comet::header::HeapObjectHeader;
use comet::heap::Heap as CometHeap;
pub use comet::internal::finalize_trait::FinalizeTrait as Finalize;
use comet::internal::gc_info::GCInfoTrait;
pub use comet::internal::trace_trait::TraceTrait as Trace;
use comet::local_heap::LocalHeap;
pub use comet::visitor::Visitor;
use mopa::mopafy;

use crate::gc::snapshot::serializer::Serializable;
pub struct Heap {
    main: Box<LocalHeap>,
    heap: CometHeap,
}

/// `GcCell` is a type that can be allocated in GC gc and passed to JavaScript environment.
///
///
/// All cells that is not part of `src/vm` treatened as dummy objects and property accesses
/// is no-op on them.
///
pub trait GcCell: mopa::Any + Serializable + Unpin {
    /// Used when object has dynamic size i.e arrays
    fn compute_size(&self) -> usize {
        std::mem::size_of_val(self)
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn deser_pair(&self) -> (usize, usize);
}

mopafy!(GcCell);

pub struct GcPointer<T: GcCell + ?Sized> {
    base: NonNull<GcPointerBase>,
    marker: PhantomData<T>,
}

#[repr(C)]
pub struct GcPointerBase {
    hdr: HeapObjectHeader,
    vtable: usize,
}

impl<T: GcCell + ?Sized> GcPointer<T> {
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
    pub fn vtable_offsetof() -> usize {
        offsetof!(GcPointerBase.vtable)
    }

    pub fn allocation_size(&self) -> usize {
        unsafe { comet::gc_size(&self.hdr) }
    }

    pub fn get_dyn(&self) -> &mut dyn GcCell {
        unsafe {
            std::mem::transmute(mopa::TraitObject {
                vtable: self.vtable as _,
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

impl<T: GcCell> std::fmt::Pointer for GcPointer<T> {
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
