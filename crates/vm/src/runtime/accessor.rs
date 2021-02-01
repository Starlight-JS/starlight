use std::mem::size_of;

use super::{
    js_cell::{allocate_cell, JsCell},
    js_value::JsValue,
    ref_ptr::AsRefPtr,
    vm::JsVirtualMachine,
};
use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

pub struct Accessor {
    getter: JsValue,
    setter: JsValue,
}

impl Accessor {
    pub fn getter(&self) -> JsValue {
        self.getter
    }

    pub fn set_getter(&mut self, val: JsValue) {
        self.getter = val;
    }

    pub fn set_setter(&mut self, val: JsValue) {
        self.setter = val;
    }

    pub fn setter(&self) -> JsValue {
        self.setter
    }
    pub fn new(
        vm: impl AsRefPtr<JsVirtualMachine>,
        getter: JsValue,
        setter: JsValue,
    ) -> Handle<Self> {
        let this = Self { getter, setter };
        allocate_cell(vm, size_of::<Self>(), this)
    }
}

impl JsCell for Accessor {}

impl HeapObject for Accessor {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        if !self.getter.is_empty() {
            self.getter.as_cell_ref_mut().visit_children(tracer);
        }
        if !self.setter.is_empty() {
            self.setter.as_cell_ref_mut().visit_children(tracer);
        }
    }

    fn needs_destruction(&self) -> bool {
        false
    }
}
