use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};
use std::mem::size_of;

use super::{
    js_cell::{allocate_cell, JsCell},
    ref_ptr::Ref,
    vm::JsVirtualMachine,
};

#[repr(C)]
pub struct JsString {
    len: u32,
    data: [u8; 0],
}

impl JsString {
    pub fn new(vm: Ref<JsVirtualMachine>, as_str: impl AsRef<str>) -> Handle<Self> {
        let str = as_str.as_ref();
        let proto = Self {
            len: str.len() as _,
            data: [],
        };
        let mut cell = allocate_cell(vm, str.len() + size_of::<Self>(), proto);

        unsafe {
            cell.len = str.len() as _;
            std::ptr::copy_nonoverlapping(
                str.as_bytes().as_ptr(),
                cell.data.as_mut_ptr(),
                str.len(),
            );
        }

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

impl HeapObject for JsString {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn compute_size(&self) -> usize {
        self.len as usize + size_of::<Self>()
    }
    fn needs_destruction(&self) -> bool {
        false
    }
}
impl JsCell for JsString {}
