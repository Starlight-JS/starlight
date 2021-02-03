use crate::{
    heap::trace::{Slot, Tracer},
    runtime::{js_cell::JsCell, structure::Structure, vm::JsVirtualMachine},
};

use super::heap_cell::{HeapCell, HeapObject};
use std::{
    collections::HashMap,
    marker::PhantomData,
    mem::transmute,
    ops::{Deref, DerefMut},
    ptr::{null_mut, NonNull},
};
use wtf_rs::TraitObject;

/// A garbage collected pointer to a value.
///
/// This is the equivalent of a garbage collected smart-pointer.
/// The objects can only survive garbage collection if they live in this smart-pointer.
///
/// The smart pointer is simply a guarantee to the garbage collector
/// that this points to a garbage collected object with the correct header,
/// and not some arbitrary bits that you've decided to heap allocate.
///
///
///
/// TODO: Implement internal pointers scanning inside GC so we can detect references to data from smart-pointer.
pub struct Handle<T: HeapObject + ?Sized> {
    pub cell: NonNull<HeapCell>,
    pub(crate) marker: PhantomData<T>,
}

impl<T: HeapObject + ?Sized> Handle<T> {
    pub fn ptr_eq<U: HeapObject + ?Sized>(this: Self, other: Handle<U>) -> bool {
        this.cell == other.cell
    }
    /// Obtains VM reference from heap allocated object.
    ///
    #[allow(clippy::mut_from_ref, clippy::needless_lifetimes)]
    pub fn vm<'a>(&'a self) -> &'a mut JsVirtualMachine {
        unsafe { &mut *(*self.cell.as_ptr()).vm().pointer }
    }
    /// Obtain VM reference from heap allocated object in fast way if object
    /// is allocated in Immix space and known to be < 8KB in size.
    ///
    /// # Safety
    /// This function is unsafe to call since it makes no checks that object
    /// is allocated inside large object space or immix space.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn vm_fast(&self) -> &mut JsVirtualMachine {
        (*self.cell.as_ptr()).fast_vm()
    }
}
impl<T: HeapObject + Sized> Handle<T> {
    /// Get GC handle from raw pointer.
    ///
    ///
    /// # Safety
    ///
    /// If `ptr` is not pointer to heap data then dereferencing returned handle might lead to segfault.
    ///
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        Self {
            cell: NonNull::new_unchecked(ptr.cast::<u8>().sub(8).cast::<HeapCell>() as *mut _),
            marker: Default::default(),
        }
    }
}
impl<T: HeapObject + ?Sized> Copy for Handle<T> {}
impl<T: HeapObject + ?Sized> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }

    fn clone_from(&mut self, source: &Self) {
        *self = *source;
    }
}

impl Handle<dyn HeapObject> {
    /// Returns the handle value, blindly assuming it to be of type `U`.
    /// If you are not *absolutely certain* of `U`, you *must not* call this.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it does not do any checks that `Handle<dyn HeapObject>` is reaully `U`.
    pub unsafe fn donwcast_unchecked<U: ?Sized + HeapObject>(self) -> Handle<U> {
        Handle {
            cell: self.cell,
            marker: PhantomData,
        }
    }
    /// Returns true if the handle type is the same as `U`
    pub fn is<U: Sized + HeapObject>(self) -> bool {
        unsafe {
            let fat_ptr: *mut dyn HeapObject = null_mut::<U>() as *mut dyn HeapObject;
            let trait_object = transmute::<_, TraitObject>(fat_ptr).vtable;
            trait_object == (*self.cell.as_ptr()).vtable().to_mut_ptr()
        }
    }
    /// Returns handle to the value if it is of type `U`, or
    /// `None` if it isn't.
    pub fn downcast<U: Sized + HeapObject>(self) -> Option<Handle<U>> {
        if self.is::<U>() {
            Some(unsafe { self.donwcast_unchecked() })
        } else {
            None
        }
    }
}

impl<T: ?Sized + HeapObject> Handle<T> {
    /// Returns dynamic handle from typed handle.
    pub fn as_dyn(self) -> Handle<dyn HeapObject> {
        Handle {
            cell: self.cell,
            marker: PhantomData,
        }
    }
}

impl<T: Sized + HeapObject> Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(*self.cell.as_ptr()).data().to_ptr::<T>() }
    }
}

impl Deref for Handle<dyn HeapObject> {
    type Target = dyn HeapObject;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(*self.cell.as_ptr()).get_dyn() }
    }
}

impl<T: Sized + HeapObject> DerefMut for Handle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(*self.cell.as_ptr()).data().to_mut_ptr::<T>() }
    }
}

impl DerefMut for Handle<dyn HeapObject> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { (*self.cell.as_ptr()).get_dyn() }
    }
}

impl<K: HeapObject, V: HeapObject> HeapObject for HashMap<K, V> {
    #[allow(mutable_transmutes)]
    #[allow(clippy::transmute_ptr_to_ptr)]
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        for (key, val) in self.iter_mut() {
            unsafe {
                transmute::<_, &mut K>(key).visit_children(tracer);
                val.visit_children(tracer);
            }
        }
    }
    fn needs_destruction(&self) -> bool {
        true
    }
}

impl<T: HeapObject> HeapObject for Handle<T> {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        tracer.trace(Slot::new(self));
    }

    fn needs_destruction(&self) -> bool {
        false
    }
}

impl<T: HeapObject> JsCell for Handle<T> {
    fn set_structure(&mut self, vm: &mut JsVirtualMachine, s: Handle<Structure>) {
        (**self).set_structure(vm, s);
    }
    fn get_structure(&self, vm: &mut JsVirtualMachine) -> Handle<Structure> {
        (**self).get_structure(vm)
    }
}

impl<K: HeapObject, V: HeapObject> JsCell for HashMap<K, V> {}

impl<T: HeapObject> JsCell for Option<T> {}
impl<T: HeapObject> HeapObject for Option<T> {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        if let Some(ref mut x) = self {
            x.visit_children(tracer);
        }
    }
    fn needs_destruction(&self) -> bool {
        std::mem::needs_drop::<T>()
    }
}

macro_rules! impl_prim {
    ($($t: ty)*) => {$(
        impl HeapObject for $t {
            fn needs_destruction(&self) -> bool {
                std::mem::needs_drop::<$t>()
            }

            fn visit_children(&mut self, _: &mut dyn Tracer) {}
        }
        impl JsCell for $t {}
    )*
    };
}

impl_prim! {
    bool
    u8 i8
    u16 i16
    u32 i32
    u64 i64
    u128 i128
    f32 f64

}
