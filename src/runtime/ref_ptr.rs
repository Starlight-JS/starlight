use std::ops::{Deref, DerefMut};

#[repr(C)]
pub struct Ref<T> {
    pointer: *mut T,
}

impl<T> Ref<T> {
    pub fn is_null(self) -> bool {
        self.pointer.is_null()
    }

    pub fn is_not_null(self) -> bool {
        !self.pointer.is_null()
    }

    pub fn new(ptr: *const T) -> Self {
        Self {
            pointer: ptr as *mut T,
        }
    }
}

impl<T> Copy for Ref<T> {}
impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Deref for Ref<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.pointer) }
    }
}

impl<T> DerefMut for Ref<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.pointer) }
    }
}

pub trait AsRefPtr<T> {
    fn as_ref_ptr(&self) -> Ref<T>;
}

impl<T> AsRefPtr<T> for &T {
    fn as_ref_ptr(&self) -> Ref<T> {
        Ref::new(*self)
    }
}

impl<T> AsRefPtr<T> for &mut T {
    fn as_ref_ptr(&self) -> Ref<T> {
        Ref::new(*self)
    }
}

impl<T> AsRefPtr<T> for Ref<T> {
    fn as_ref_ptr(&self) -> Ref<T> {
        *self
    }
}
