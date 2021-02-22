use super::heap::Heap;
use crate::gc::cell::{Trace, Tracer};
use std::ops::{Deref, DerefMut};

pub struct HandleInner<T: Trace> {
    value: T,
    rc: u32,
    heap: *mut Heap,
}

pub trait HandleTrait: Trace {}
unsafe impl<T: Trace> Trace for HandleInner<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.value.trace(tracer);
    }
}
impl<T: Trace> HandleTrait for HandleInner<T> {}

/// Generic reference counted structure used by GC to trace stored value for GC objects.
///
///
/// This type should always be used for `Gc<T>` types on stack so GC can see GC objects and mark then, not using
/// `Handle<Gc<T>>` might result in UB or segfaults.
///
pub struct Handle<T: Trace> {
    inner: *mut HandleInner<T>,
}

impl<T: Trace> Handle<T> {
    /// Create new handle from `value`. On each GC cycle `value` will be scanned through `T::trace` method.
    pub fn new(heap: &mut Heap, value: T) -> Handle<T> {
        unsafe {
            let mem = Box::into_raw(Box::new(HandleInner {
                value,
                rc: 1,
                heap: heap as *mut _,
            }));
            (*heap).handles.insert(mem as *mut _);
            Self { inner: mem }
        }
    }
    fn inner(&self) -> &mut HandleInner<T> {
        unsafe { &mut *self.inner }
    }
    #[inline(never)]
    fn destroy(&mut self) {
        unsafe {
            let heap = self.inner().heap;
            (*heap)
                .handles
                .remove(&(self.inner as *mut dyn HandleTrait));
            let _ = Box::from_raw(self.inner);
        }
    }
}

impl<T: Trace> Drop for Handle<T> {
    fn drop(&mut self) {
        let inner = self.inner();
        if inner.rc == 1 {
            self.destroy();
        } else {
            inner.rc -= 1;
        }
    }
}

impl<T: Trace> Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T: Trace> DerefMut for Handle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner().value
    }
}
impl<T: Trace> Clone for Handle<T> {
    fn clone(&self) -> Self {
        self.inner().rc += 1;
        Self { inner: self.inner }
    }
}
