#![no_std]

use core::hint::unreachable_unchecked;

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

pub fn unwrap_unchecked<T>(option: Option<T>) -> T {
    match option {
        Some(value) => value,
        None => unsafe { core::hint::unreachable_unchecked() },
    }
}
