use crate::{
    gc::cell::{Cell, Gc, Trace, Tracer},
    vm::VirtualMachine,
};

use super::value::JsValue;

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
    pub fn new(vm: &mut VirtualMachine, getter: JsValue, setter: JsValue) -> Gc<Self> {
        let this = Self { getter, setter };
        vm.space().alloc(this)
    }
}

impl Cell for Accessor {}

unsafe impl Trace for Accessor {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.setter.trace(tracer);
        self.getter.trace(tracer);
    }
}
