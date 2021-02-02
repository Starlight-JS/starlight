//! # wtf-rs
//!
//! This crate is inspired by WebKit WTF (WTF - Web Template Framework) and it just includes some nice
//! extras to Rust std lib. Note that this crate does not fully rewrites WTF in Rust but consists of some
//! additional functions that is used across `starlight` engine.
//!
//!

#![no_std]

use core::{cmp::Ordering, ptr::write_volatile};

/// Get offset of field in type. This works for multiple fields e.g `x.y.z`.
#[macro_export]
macro_rules! object_offsetof {
    ($type: ty,$($field: ident).*) => {{
        unsafe {
            let pointer = 0x400usize as *const $type;
            let field_pointer = &(*pointer).$($field).* as *const _ as usize;
            field_pointer - 0x400
        }
    }};
}

#[macro_export]
macro_rules! lohi_struct {
    (struct $name : ident {
        $field1: ident : $t: ty,
        $field2: ident : $t2: ty,
    }) => {
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[repr(C)]
        #[cfg(target_endian = "big")]
        pub struct $name {
            pub $field2: $t2,
            pub $field1: $t,
        }
        #[derive(Copy, Clone, PartialEq, Eq)]
        #[repr(C)]
        #[cfg(target_endian = "little")]
        pub struct $name {
            pub $field1: $t,
            pub $field2: $t,
        }
    };
}
pub mod cryptographically_random_number;
pub mod random_device;
pub mod random_number;
pub mod stack_bounds;
pub mod tagged_pointer;
pub mod weak_random;
#[repr(C)]
pub struct TraitObject {
    pub data: *mut (),
    pub vtable: *mut (),
}

pub(crate) fn thread_self() -> u64 {
    #[cfg(windows)]
    unsafe {
        extern "C" {
            fn GetCurrentThreadId() -> u32;
        }
        GetCurrentThreadId() as u64
    }
    #[cfg(unix)]
    unsafe {
        libc::pthread_self() as u64
    }
}

/// Does unchecked unwrap of `Option<T>` value. This is used when you're really sure that some option
/// holds value.
pub fn unwrap_unchecked<T>(option: Option<T>) -> T {
    match option {
        Some(value) => value,
        None => unsafe { core::hint::unreachable_unchecked() },
    }
}
static mut SINK: usize = 0;
/// `keep_on_stack!` internally used function. This function just does simple volatile write
/// to global variable so compiler does not optimize `value` out.
pub fn keep_on_stack_noop(value: usize) {
    unsafe {
        write_volatile(&mut SINK, value);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// Forces Rust to keep variables or some other values on stack. Note that this macro
/// expects references to values as input.
///
/// # Example
/// ```rust,ignore
/// use wtf_rs::keep_on_stack;
///
///
/// let x = 42;
/// let mut y = 3;
/// keep_on_stack!(&x,&mut y);
/// ```
#[macro_export]
macro_rules! keep_on_stack {
    ($($value : expr),*) => {{
        $(
            $crate::keep_on_stack_noop($value as *const _ as usize);
        )*
    }};
}
