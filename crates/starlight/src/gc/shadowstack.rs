//! Shadow stack implementation for object rooting.
//!
//!
//! # Description
//! Unlike other algorithms to take care of rooted objects like use reference counting to take count of instances
//! of stack, this algorithm maintains a singly linked list of stack roots. This so-called "shadow stack" mirrors the
//! machine stack. Maintaining this data is much faster and memory-efficent than using reference-counted stack roots,
//! it does not require heap allocation, and does not rely on compiler optimizations.
//!
//!
//!
//!
use crate::gc::cell::{Trace, Tracer};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::{cell::Cell, ptr::null_mut};

/// Shadow stack type. This is a simple sinly-linked list used for rooting in starlight.
pub struct ShadowStack {
    #[doc(hidden)]
    pub head: Cell<*mut RawShadowStackEntry>,
}

#[repr(C)]
pub struct RawShadowStackEntry {
    /// Shadowstack itself
    stack: *mut ShadowStack,
    /// Previous rooted entry
    prev: *mut RawShadowStackEntry,
    /// Pointer to vtable that is a `Trace` of rooted variable
    vtable: usize,
    /// Value is located right after vtable pointer, to access it we can construct trait object.
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

impl<'a, T: Trace> RootedInternal<'a, T> {
    #[inline]
    pub unsafe fn construct(
        stack: &'a ShadowStack,
        prev: *mut RawShadowStackEntry,
        vtable: usize,
        value: T,
    ) -> Self {
        Self {
            stack,
            prev,
            vtable,
            value,
        }
    }
}

/// Rooted value on stack. This is non-copyable type that is used to hold GC thing on stack.
///
/// # Usage
///
/// This type must be used for any type that is GC pointer or contains pointers to GC objects.
/// To construct rooted value [root!](root) macro should be used with provided shadowstack.
///
pub struct Rooted<'a, 'b, T: Trace> {
    #[doc(hidden)]
    pinned: std::pin::Pin<&'a mut RootedInternal<'b, T>>,
}
pub fn identity_clos<T, R>(x: T, clos: impl FnOnce(T) -> R) -> R {
    clos(x)
}
/// Create [Rooted<T>](Rooted) instance and push it to provided shadowstack instance.
///
///
/// ***NOTE***: This macro does not heap allocate internally. It uses some unsafe tricks to
/// allocate value on stack and push stack reference to shadowstack. Returned `Rooted<T>` internally
/// is `Pin<&mut T>`.
///
#[macro_export]
macro_rules! root {
    ($name: ident: $t: ty  = $stack: expr,$value: expr) => {
        let stack: &ShadowStack = &$stack;
        let value = $value;
        let mut $name = unsafe {
            $crate::gc::shadowstack::RootedInternal::<$t>::construct(
                stack as *mut _,
                stack.head,
                std::mem::transmute::<_, mopa::TraitObject>(&value as &dyn $crate::gc::cell::Trace)
                    .vtable as usize,
                value,
            )
        };

        stack.head.set(unsafe { std::mem::transmute(&mut $name) });

        let mut $name =
            unsafe { $crate::gc::shadowstack::Rooted::construct(std::pin::Pin::new(&mut $name)) };
    };

    ($name : ident = $stack: expr,$value: expr) => {
        let stack: &$crate::gc::shadowstack::ShadowStack = &$stack;
        let value = $value;
        let mut $name = unsafe {
            $crate::gc::shadowstack::RootedInternal::<_>::construct(
                stack,
                stack.head.get(),
                std::mem::transmute::<_, mopa::TraitObject>(&value as &dyn $crate::gc::cell::Trace)
                    .vtable as usize,
                value,
            )
        };

        stack.head.set(unsafe { std::mem::transmute(&mut $name) });

        let mut $name =
            unsafe { $crate::gc::shadowstack::Rooted::construct(std::pin::Pin::new(&mut $name)) };
    };
}

impl<'a, 'b, T: Trace> Rooted<'a, 'b, T> {
    /// Create `Rooted<T>` instance from pinned reference. Note that this should be used only
    /// inside `root!` macro and users of Starlight API should not use this function.
    pub unsafe fn construct(pin: Pin<&'a mut RootedInternal<'b, T>>) -> Self {
        Self { pinned: pin }
    }
    pub unsafe fn get_internal(&self) -> &RootedInternal<T> {
        std::mem::transmute_copy::<_, &RootedInternal<T>>(&self.pinned)
    }
    pub unsafe fn get_internal_mut(&mut self) -> &mut RootedInternal<T> {
        std::mem::transmute_copy::<_, &mut RootedInternal<T>>(&mut self.pinned)
    }

    pub fn mut_handle(&mut self) -> HandleMut<'_, T> {
        HandleMut { value: &mut **self }
    }

    pub fn handle(&self) -> Handle<'_, T> {
        Handle { value: &**self }
    }
}

impl<'a, T: Trace> Deref for Rooted<'a, '_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.pinned.value
    }
}

impl<'a, T: Trace> DerefMut for Rooted<'a, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut std::mem::transmute_copy::<_, &mut RootedInternal<T>>(&mut self.pinned).value
        }
    }
}

/// Reference to `Rooted<T>` value.
pub struct Handle<'a, T: Trace> {
    value: &'a T,
}

pub struct HandleMut<'a, T: Trace> {
    value: &'a mut T,
}
impl<T: Trace> Deref for Handle<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T: Trace> Deref for HandleMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}
impl<T: Trace> DerefMut for HandleMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}
