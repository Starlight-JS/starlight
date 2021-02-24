pub mod stack_bounds;
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
static mut SINK: usize = 0;
/// `keep_on_stack!` internally used function. This function just does simple volatile write
/// to global variable so compiler does not optimize `value` out.
pub fn keep_on_stack_noop(value: usize) {
    unsafe {
        core::ptr::write_volatile(&mut SINK, value);
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
