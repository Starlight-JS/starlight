use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use wtf_rs::list::LinkedList;

use super::cell::Trace;
use crate::gc::space::Space;
pub struct LocalContextInner {
    pub prev: *mut Self,
    pub next: *mut Self,
    pub space: *mut Space,
    pub roots: LinkedList<Option<NonNull<dyn Trace>>>,
}
pub struct LocalContext<'a> {
    pub(crate) inner: NonNull<Option<NonNull<LocalContextInner>>>,
    pub(crate) marker: PhantomData<&'a ()>,
}

pub struct Local<'a, T: Trace> {
    val: *mut Option<NonNull<dyn Trace>>,
    _marker: PhantomData<&'a T>,
}
pub struct PersistentContext {
    pub(crate) inner: *mut LocalContextInner,
}

impl<T: Trace> Drop for Local<'_, T> {
    fn drop(&mut self) {
        unsafe {
            let mem = self.val.read().unwrap();

            let _ = Box::from_raw(mem.as_ptr());
            self.val.write(None);
        }
    }
}

impl<'a> Drop for LocalContext<'a> {
    fn drop(&mut self) {
        unsafe {
            while let Some(root) = self.inner().roots.pop_front() {
                match root {
                    Some(mem) => {
                        let _ = Box::from_raw(mem.as_ptr());
                    }
                    _ => (),
                }
            }

            let _ = Box::from_raw((*self.inner.as_ptr()).unwrap().as_ptr());
            (*self.inner.as_ptr()) = None;
        }
    }
}

impl PersistentContext {
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut LocalContextInner {
        unsafe { &mut *self.inner }
    }
}
impl<'a> LocalContext<'a> {
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut LocalContextInner {
        unsafe { &mut *(&mut *self.inner.as_ptr()).unwrap().as_ptr() }
    }

    pub fn new_local<T: Trace + Sized + 'static>(&'a self, value: T) -> Local<'a, T> {
        unsafe {
            let mem = Box::into_raw(Box::new(value)) as *mut dyn Trace;

            self.inner()
                .roots
                .push_back(Some(NonNull::new_unchecked(mem)));

            Local {
                val: self.inner().roots.back_mut().unwrap(),
                _marker: Default::default(),
            }
        }
    }

    pub fn escape<'x, T: Trace + Sized + 'static>(
        &mut self,
        upper: &mut LocalContext<'x>,
        value: Local<'a, T>,
    ) -> Local<'x, T> {
        unsafe {
            let x = value.val.read().take();
            upper.inner().roots.push_back(x);
            Local {
                val: upper.inner().roots.back_mut().unwrap(),
                _marker: Default::default(),
            }
        }
    }

    pub fn escape_persistent<'b, T: Trace + 'static>(
        &'b mut self,
        value: Local<'b, T>,
    ) -> Local<'static, T> {
        unsafe {
            let space = &mut *self.inner().space;
            let upper = space.persistent_context();

            let x = value.val.read().take();
            upper.inner().roots.push_back(x);
            Local {
                val: upper.inner().roots.back_mut().unwrap(),
                _marker: Default::default(),
            }
        }
    }
}

impl PersistentContext {
    pub fn new_local<T: Trace + Sized + 'static>(&mut self, value: T) -> Local<'static, T> {
        unsafe {
            let mem = Box::into_raw(Box::new(value)) as *mut dyn Trace;

            self.inner()
                .roots
                .push_back(Some(NonNull::new_unchecked(mem)));

            Local {
                val: self.inner().roots.back_mut().unwrap(),
                _marker: Default::default(),
            }
        }
    }
}

impl<'a, T: Trace> Deref for Local<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe {
            let cell = self.val.read().unwrap().as_ptr();
            (*cell).downcast_mut_unchecked()
        }
    }
}

impl<'a, T: Trace> DerefMut for Local<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let cell = self.val.read().unwrap().as_ptr();
            (*cell).downcast_mut_unchecked()
        }
    }
}
