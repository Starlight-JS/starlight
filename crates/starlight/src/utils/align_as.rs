/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use core::ops::{Deref, DerefMut};
use std::mem::ManuallyDrop;

/// Wrap an object of type T to give it the alignment requirements of an object of type A.
///
/// This has a similar effect to #[repr(align(...))], but is parameterized to support the creation
/// of structures that are generic over alignment.
///
/// Example:
/// ```rust,ignore
/// extern crate alignas;
/// use alignas::AlignAs;
/// use std::{mem, slice};
///
/// // put some byte data into the buffer
/// let mut buffer: AlignAs<_, u64> = AlignAs::new([0u8; 64]);
/// buffer[3..18].copy_from_slice(b"some input here");
///
/// // now do something with it that requires aligned access...
/// let mut ints = [0u64; 8];
/// let ptr = &buffer as *const _ as *const u64;
/// ints.copy_from_slice(unsafe { slice::from_raw_parts(ptr, 8) });
/// ```
#[repr(C)]
pub union AlignAs<T: Copy, A> {
    t: T,
    _marker: ManuallyDrop<A>,
}

impl<T: Copy, A> AlignAs<T, A> {
    #[inline]
    pub fn new(t: T) -> Self {
        AlignAs { t }
    }

    #[inline]
    pub fn raw(&self) -> *const T {
        self as *const Self as *const T
    }
}

impl<T: Copy, A: Copy> Deref for AlignAs<T, A> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &self.t }
    }
}

impl<T: Copy, A: Copy> DerefMut for AlignAs<T, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.t }
    }
}
