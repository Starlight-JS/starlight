use crate::heap::cell::{Trace, Tracer};
use std::ops::{Deref, DerefMut};
use std::{cell::Cell, ptr::null_mut};

pub struct ShadowStack {
    #[doc(hidden)]
    pub head: Cell<*mut RawShadowStackEntry>,
}

#[repr(C)]
pub struct RawShadowStackEntry {
    stack: *mut ShadowStack,
    prev: *mut RawShadowStackEntry,
    vtable: usize,
    data_start: [u8; 0],
}
impl RawShadowStackEntry {
    pub unsafe fn get_dyn(&self) -> &mut dyn Trace {
        std::mem::transmute(mopa::TraitObject {
            vtable: self.vtable as _,
            data: self.data_start.as_ptr() as *mut (),
        })
    }
}
impl ShadowStack {
    pub fn new() -> Self {
        Self {
            head: Cell::new(null_mut()),
        }
    }
}

unsafe impl Trace for ShadowStack {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        unsafe {
            let mut head = *self.head.as_ptr();
            while !head.is_null() {
                let next = (*head).prev;
                (*head).get_dyn().trace(visitor);
                head = next;
            }
        }
    }
}

impl<T: Trace> Drop for RootedInternal<'_, T> {
    fn drop(&mut self) {
        (*self.stack).head.set(self.prev);
    }
}

#[repr(C)]
pub struct RootedInternal<'a, T: Trace> {
    pub stack: &'a ShadowStack,
    pub prev: *mut RawShadowStackEntry,
    pub vtable: usize,
    pub value: T,
}

pub struct Rooted<'a, 'b, T: Trace> {
    #[doc(hidden)]
    pub pinned: std::pin::Pin<&'a mut RootedInternal<'b, T>>,
}

#[macro_export]
macro_rules! root {
    ($name: ident: $t: ty  = $stack: expr,$value: expr) => {
        let stack: &ShadowStack = &$stack;
        let value = $value;
        let mut $name = $crate::gc::shadowstack::RootedInternal::<$t> {
            stack: stack as *mut _,
            prev: stack.head,
            vtable: unsafe {
                std::mem::transmute::<_, mopa::TraitObject>(
                    &value as &dyn $crate::heap::cell::Trace,
                )
                .vtable as usize
            },
            value,
        };

        stack.head.set(unsafe { std::mem::transmute(&mut $name) });

        let mut $name = $crate::gc::shadowstack::Rooted {
            pinned: std::pin::Pin::new(&mut $name),
        };
    };

    ($name : ident = $stack: expr,$value: expr) => {
        let stack: &$crate::gc::shadowstack::ShadowStack = &$stack;
        let value = $value;
        let mut $name = $crate::gc::shadowstack::RootedInternal::<_> {
            prev: stack.head.get(),
            stack,
            vtable: unsafe {
                std::mem::transmute::<_, mopa::TraitObject>(
                    &value as &dyn $crate::heap::cell::Trace,
                )
                .vtable as usize
            },
            value,
        };

        stack.head.set(unsafe { std::mem::transmute(&mut $name) });

        let mut $name = $crate::gc::shadowstack::Rooted {
            pinned: std::pin::Pin::new(&mut $name),
        };
    };
}

impl<'a, T: Trace> Rooted<'a, '_, T> {
    pub unsafe fn get_internal(&self) -> &RootedInternal<T> {
        std::mem::transmute_copy::<_, &RootedInternal<T>>(&self.pinned)
    }
    pub unsafe fn get_internal_mut(&mut self) -> &mut RootedInternal<T> {
        std::mem::transmute_copy::<_, &mut RootedInternal<T>>(&mut self.pinned)
    }
}

impl<'a, T: Trace> Deref for Rooted<'a, '_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &std::mem::transmute_copy::<_, &RootedInternal<T>>(&self.pinned).value }
    }
}

impl<'a, T: Trace> DerefMut for Rooted<'a, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut std::mem::transmute_copy::<_, &mut RootedInternal<T>>(&mut self.pinned).value
        }
    }
}
