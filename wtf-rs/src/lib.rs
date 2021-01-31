#![no_std]

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
