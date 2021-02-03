pub mod address;
pub mod tagged_pointer;
#[macro_use]
pub mod bitmap_gen;
#[macro_export]
macro_rules! as_atomic {
    ($value: expr;$t: ident) => {
        unsafe { &*($value as *const _ as *const core::sync::atomic::$t) }
    };
}

/// Creates new zeroed value of `T`.
///
/// # Safety
///
/// Since it returns zeroed value it is almost the same as uninitialized.
pub unsafe fn zeroed<T>() -> T {
    core::mem::MaybeUninit::<T>::zeroed().assume_init()
}

pub const fn round_down(x: u64, n: u64) -> u64 {
    x & !n
}

pub const fn round_up(x: u64, n: u64) -> u64 {
    round_down(x + n - 1, n)
}

/// rounds the given value `val` up to the nearest multiple
/// of `align`.
pub fn align_usize(value: usize, align: usize) -> usize {
    if align == 0 {
        return value;
    }

    ((value + align - 1) / align) * align
}
