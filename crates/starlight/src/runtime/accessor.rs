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

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for Accessor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let x = serializer.serialize_struct("Accessor", 2)?;
        x.serialize_field("getter", &self.getter)?;
        x.serialize_field("setter", &self.setter)?;
        x.end()
    }
}
