use crate::{
    heap::cell::{Cell, Gc, Trace, Tracer},
    vm::VirtualMachine,
};

use super::{gc_array::GcArray, value::JsValue};

pub struct Arguments {
    pub this: JsValue,
    pub values: Gc<GcArray<JsValue>>,
    pub ctor_call: bool,
}

impl Arguments {
    pub fn size(&self) -> usize {
        self.values.len()
    }
    pub fn new(vm: &mut VirtualMachine, this: JsValue, size: usize) -> Self {
        let arr = GcArray::new(vm.space(), size, JsValue::undefined());
        Self {
            this,
            values: arr,
            ctor_call: false,
        }
    }
    pub fn at_mut(&mut self, x: usize) -> &mut JsValue {
        if x < self.size() {
            &mut self.values[x]
        } else {
            panic!("Out of bounds arguments");
        }
    }
    pub fn at(&self, x: usize) -> JsValue {
        if x < self.size() {
            self.values[x]
        } else {
            JsValue::undefined()
        }
    }
}

impl Cell for Arguments {}
unsafe impl Trace for Arguments {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.this.trace(tracer);
        self.values.trace(tracer);
    }
}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for Arguments {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let x = serializer.serialize_struct("Arguments", 2)?;
        x.serialize_field("this", &self.this);
        x.serialize_field("values", &self.values);
        x.end()
    }
}
