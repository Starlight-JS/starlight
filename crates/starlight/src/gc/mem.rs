/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::i32;
use std::mem::size_of;

use crate::gc::os;

/// return pointer width: either 4 or 8
/// (although only 64bit architectures are supported right now)
#[inline(always)]
pub fn ptr_width() -> i32 {
    size_of::<*const u8>() as i32
}

#[inline(always)]
pub fn ptr_width_usize() -> usize {
    size_of::<*const u8>() as usize
}

/// returns true if given value is a multiple of a page size.
pub fn is_page_aligned(val: usize) -> bool {
    let align = os::page_size_bits();

    // we can use shifts here since we know that
    // page size is power of 2
    val == ((val >> align) << align)
}

#[test]
fn test_is_page_aligned() {
    let p = os::page_size();

    assert_eq!(false, is_page_aligned(1));
    assert_eq!(false, is_page_aligned(2));
    assert_eq!(false, is_page_aligned(64));
    assert_eq!(true, is_page_aligned(p));
    assert_eq!(true, is_page_aligned(2 * p));
    assert_eq!(true, is_page_aligned(3 * p));
}

/// round the given value up to the nearest multiple of a page
pub fn page_align(val: usize) -> usize {
    let align = os::page_size_bits();

    // we know that page size is power of 2, hence
    // we can use shifts instead of expensive division
    ((val + (1 << align) - 1) >> align) << align
}

#[test]
fn test_page_align() {
    let p = os::page_size();

    assert_eq!(p, page_align(1));
    assert_eq!(p, page_align(p - 1));
    assert_eq!(p, page_align(p));
    assert_eq!(2 * p, page_align(p + 1));
}

/// rounds the given value `val` up to the nearest multiple
/// of `align`
pub fn align(value: u32, align: u32) -> u32 {
    if align == 0 {
        return value;
    }

    ((value + align - 1) / align) * align
}

/// rounds the given value `val` up to the nearest multiple
/// of `align`
pub fn align_i32(value: i32, align: i32) -> i32 {
    if align == 0 {
        return value;
    }

    ((value + align - 1) / align) * align
}

/// rounds the given value `val` up to the nearest multiple
/// of `align`.
pub fn align_usize(value: usize, align: usize) -> usize {
    if align == 0 {
        return value;
    }

    ((value + align - 1) / align) * align
}

/// returns 'true' if th given `value` is already aligned
/// to `align`.
pub fn is_aligned(value: usize, align: usize) -> bool {
    align_usize(value, align) == value
}

/// returns true if value fits into u8 (unsigned 8bits).
pub fn fits_u8(value: i64) -> bool {
    (0..=255).contains(&value)
}

/// returns true if value fits into i32 (signed 32bits).
pub fn fits_i32(value: i64) -> bool {
    i32::MIN as i64 <= value && value <= i32::MAX as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fits_u8() {
        assert_eq!(true, fits_u8(0));
        assert_eq!(true, fits_u8(255));
        assert_eq!(false, fits_u8(256));
        assert_eq!(false, fits_u8(-1));
    }

    #[test]
    fn test_fits_i32() {
        assert_eq!(true, fits_i32(0));
        assert_eq!(true, fits_i32(i32::MAX as i64));
        assert_eq!(true, fits_i32(i32::MIN as i64));
        assert_eq!(false, fits_i32(i32::MAX as i64 + 1));
        assert_eq!(false, fits_i32(i32::MIN as i64 - 1));
    }
}
