use crate::{
    heap::trace::{Slot, Tracer},
    runtime::js_cell::JsCell,
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

pub struct Handle<T: HeapObject + ?Sized> {
    pub(crate) cell: NonNull<HeapCell>,
    pub(crate) marker: PhantomData<T>,
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
    pub unsafe fn donwcast_unchecked<U: ?Sized + HeapObject>(self) -> Handle<U> {
        Handle {
            cell: self.cell,
            marker: PhantomData,
        }
    }
    pub fn is<U: Sized + HeapObject>(self) -> bool {
        unsafe {
            let fat_ptr: *mut dyn HeapObject = null_mut::<U>() as *mut dyn HeapObject;
            let trait_object = transmute::<_, TraitObject>(fat_ptr).vtable;
            trait_object == (*self.cell.as_ptr()).vtable().to_mut_ptr()
        }
    }
    pub fn downcast<U: Sized + HeapObject>(self) -> Option<Handle<U>> {
        if self.is::<U>() {
            return Some(unsafe { self.donwcast_unchecked() });
        } else {
            None
        }
    }
}

impl<T: ?Sized + HeapObject> Handle<T> {
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

impl<T: HeapObject> JsCell for Handle<T> {}

impl<K: HeapObject, V: HeapObject> JsCell for HashMap<K, V> {}

impl<T: HeapObject> JsCell for Option<T> {}
impl<T: HeapObject> HeapObject for Option<T> {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        match self {
            Some(ref mut x) => x.visit_children(tracer),
            _ => (),
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
