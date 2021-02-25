use std::mem::size_of;

use crate::heap::cell::{GcCell, GcPointer, Trace};

use super::Runtime;

#[repr(C)]
pub struct JsString {
    cap: u32,
    len: u32,
    data: [u8; 0],
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn new(vm: &mut Runtime, as_str: impl AsRef<str>) -> GcPointer<Self> {
        let str = as_str.as_ref();
        let proto = Self {
            cap: str.len() as _,
            len: str.len() as _,
            data: [],
        };
        let cell = vm.heap().allocate(proto);

        cell
    }

    pub fn as_str(&self) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                self.data.as_ptr(),
                self.len as _,
            ))
        }
    }

    pub fn len(&self) -> u32 {
        self.len
    }
}

unsafe impl Trace for JsString {}
impl GcCell for JsString {
    fn compute_size(&self) -> usize {
        self.cap as usize + size_of::<Self>()
    }
}
