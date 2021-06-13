// TODO: Use mimalloc there?
use crate::prelude::*;
use std::{intrinsics::unlikely, ptr::null_mut};
pub struct JsArrayBuffer {
    data: *mut u8,
    size: usize,
    attached: bool,
}

impl JsArrayBuffer {
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn attached(&self) -> bool {
        self.attached
    }

    pub fn data(&self) -> &[u8] {
        assert!(!self.data.is_null());
        unsafe { std::slice::from_raw_parts(self.data, self.size()) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        assert!(!self.data.is_null());
        unsafe { std::slice::from_raw_parts_mut(self.data, self.size()) }
    }

    pub fn detach(&mut self) {
        if !self.data.is_null() {
            unsafe {
                libc::free(self.data.cast());
                self.data = null_mut();
                self.size = 0;
            }
        }
        self.attached = false;
    }

    pub fn create_data_block(
        &mut self,
        rt: &mut Runtime,
        size: usize,
        zero: bool,
    ) -> Result<(), JsValue> {
        self.detach();
        if size == 0 {
            self.attached = true;
            return Ok(());
        }

        if unlikely(size > u32::MAX as usize) {
            let msg = JsString::new(rt, "Cannot allocate a data block for the ArrayBuffer");
            return Err(JsValue::new(JsRangeError::new(rt, msg, None)));
        }
        unsafe {
            self.data = if zero {
                libc::calloc(1, size).cast()
            } else {
                libc::malloc(size).cast()
            };

            if unlikely(self.data.is_null()) {
                let msg = JsString::new(rt, "Cannot allocate a data block for the ArrayBuffer");
                return Err(JsValue::new(JsRangeError::new(rt, msg, None)));
            }
            self.attached = true;
            self.size = size;
        }
        Ok(())
    }
}
