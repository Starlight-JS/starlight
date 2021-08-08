use crate::{
    gc::cell::{GcCell, GcPointer},
    vm::{code_block::CodeBlock, value::*},
};

use super::callframe::CallFrame;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Register {
    u: RegisterPayload,
}
#[derive(Clone, Copy)]
#[repr(C)]
pub union RegisterPayload {
    pub value: JsValue,
    pub call_frame: *mut CallFrame,
    pub code_block: Option<GcPointer<CodeBlock>>,
    pub number: f64,
    pub integer: i64,
}

impl Register {
    pub fn code_block(self) -> Option<GcPointer<CodeBlock>> {
        unsafe { self.u.code_block }
    }
    pub fn new(val: JsValue) -> Self {
        Self {
            u: RegisterPayload { value: val },
        }
    }

    pub fn js_val(self) -> JsValue {
        unsafe { self.u.value }
    }
    pub fn i(self) -> i32 {
        self.js_val().get_int32()
    }
    pub fn payload(self) -> i32 {
        unsafe { self.js_val().0.as_bits.payload }
    }

    pub fn unboxed_int32(self) -> i32 {
        self.payload()
    }

    pub fn unboxed_uint32(self) -> u32 {
        self.payload() as u32
    }
    pub fn unboxed_int52(self) -> i64 {
        unsafe { self.u.integer >> 12 }
    }
    pub fn unboxed_strict_int52(self) -> i64 {
        unsafe { self.u.integer }
    }
    pub fn unboxed_int64(self) -> i64 {
        unsafe { self.u.integer }
    }
    pub fn unboxed_boolean(self) -> bool {
        unsafe { self.u.integer != 0 }
    }
    pub fn unboxed_double(self) -> f64 {
        unsafe { self.u.number }
    }
    pub fn unboxed_cell(self) -> GcPointer<dyn GcCell> {
        #[cfg(target_pointer_width = "64")]
        unsafe {
            self.js_val().get_object()
        }
        #[cfg(target_pointer_width = "32")]
        unsafe {
            std::mem::transmute(self.payload())
        }
    }

    pub fn pointer(self) -> *mut u8 {
        #[cfg(target_pointer_width = "64")]
        unsafe {
            self.unboxed_int64() as _
        }
        #[cfg(target_pointer_width = "32")]
        unsafe {
            std::mem::transmute(self.payload())
        }
    }
    pub fn tag(self) -> i32 {
        unsafe { self.js_val().0.as_bits.tag }
    }
    pub fn tag_mut(&mut self) -> &mut i32 {
        unsafe { &mut self.u.value.0.as_bits.tag }
    }
    pub fn payload_mut(&mut self) -> &mut i32 {
        unsafe { &mut self.u.value.0.as_bits.payload }
    }
}
