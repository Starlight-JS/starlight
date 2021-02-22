use crate::{
    gc::{
        cell::{Cell, Gc, Trace, Tracer},
        handle::Handle,
    },
    vm::VirtualMachine,
};

use super::{arguments::Arguments, value::JsValue};

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

    pub fn invoke_getter(
        &self,
        vm: &mut VirtualMachine,
        this_binding: JsValue,
    ) -> Result<JsValue, JsValue> {
        if self.getter().is_callable() {
            let args = Arguments::new(vm, this_binding, 0);
            let mut args = Handle::new(vm.space(), args);
            self.getter()
                .as_object()
                .as_function_mut()
                .call(vm, &mut args)
        } else {
            Ok(JsValue::undefined())
        }
    }
}

impl Cell for Accessor {}

unsafe impl Trace for Accessor {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.setter.trace(tracer);
        self.getter.trace(tracer);
    }
}
