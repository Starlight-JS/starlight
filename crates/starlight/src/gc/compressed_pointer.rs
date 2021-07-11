use crate::{
    prelude::{Deserializable, Serializable},
    vm::Runtime,
};

use super::cell::{GcCell, GcPointer, Trace};
use std::marker::PhantomData;

#[repr(transparent)]
pub struct CompressedPtr<T: GcCell + ?Sized> {
    marker: PhantomData<T>,
    #[cfg(feature = "compressed-pointers")]
    pointer: u32,
    #[cfg(not(feature = "compressed-pointers"))]
    pointer: GcPointer<T>,
}
#[allow(unused_variables)]
impl<T: GcCell + ?Sized> CompressedPtr<T> {
    #[inline(always)]
    pub fn new<PAGE: HeapPage>(page: &PAGE, gcptr: GcPointer<T>) -> Self {
        #[cfg(feature = "compressed-pointers")]
        let compressed = page.compress(gcptr.as_dyn());
        #[cfg(not(feature = "compressed-pointers"))]
        let compressed = gcptr;
        Self {
            pointer: compressed,
            marker: PhantomData,
        }
    }
    #[inline(always)]
    pub fn get<PAGE: HeapPage>(&self, page: &PAGE) -> GcPointer<T> {
        #[cfg(feature = "compressed-pointers")]
        {
            unsafe { page.decompress(self.pointer) }
        }
        #[cfg(not(feature = "compressed-pointers"))]
        {
            self.pointer
        }
    }
    #[inline(always)]
    pub fn ptr_eq<U: GcCell + ?Sized>(&self, other: &CompressedPtr<U>) -> bool {
        #[cfg(feature = "compressed-pointers")]
        {
            self.pointer == other.pointer
        }
        #[cfg(not(feature = "compressed-pointers"))]
        {
            GcPointer::ptr_eq(&self.pointer, &other.pointer)
        }
    }

    pub fn as_dyn(&self) -> CompressedPtr<dyn GcCell> {
        CompressedPtr {
            #[cfg(feature = "compressed-pointers")]
            pointer: self.pointer,
            #[cfg(not(feature = "compressed-pointers"))]
            pointer: self.pointer.as_dyn(),
            marker: PhantomData,
        }
    }
}
pub trait HeapPage {
    fn compress(&self, ptr: GcPointer<dyn GcCell>) -> u32;
    fn decompress<T: ?Sized + GcCell>(&self, compressed: u32) -> GcPointer<T>;
}

impl<T: GcCell + ?Sized> Copy for CompressedPtr<T> {}
impl<T: GcCell + ?Sized> Clone for CompressedPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GcCell + ?Sized> Serializable for CompressedPtr<T> {
    fn serialize(&self, serializer: &mut crate::prelude::SnapshotSerializer) {
        self.get(&*serializer.rt()).serialize(serializer);
    }
}
impl<T: GcCell + ?Sized> Deserializable for CompressedPtr<T> {
    unsafe fn deserialize_inplace(deser: &mut crate::prelude::Deserializer) -> Self {
        let rt = &mut *deser.rt;
        let ref_ = GcPointer::<T>::deserialize_inplace(deser);
        Self::new(rt, ref_)
    }
    unsafe fn deserialize(_at: *mut u8, _deser: &mut crate::prelude::Deserializer) {
        unreachable!()
    }
    unsafe fn allocate(
        _rt: &mut Runtime,
        _deser: &mut crate::prelude::Deserializer,
    ) -> *mut super::cell::GcPointerBase {
        unreachable!()
    }
}

unsafe impl<T: GcCell + ?Sized> Trace for CompressedPtr<T> {
    fn trace(&mut self, visitor: &mut dyn super::cell::Tracer) {
        visitor.visit_compressed(self.as_dyn());
    }
}

impl<T: GcCell + ?Sized> GcCell for CompressedPtr<T> {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

impl<T: GcCell + ?Sized> PartialEq for CompressedPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl<T: GcCell + ?Sized> Eq for CompressedPtr<T> {}
