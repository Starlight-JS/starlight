use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
    util::array::GcVec,
};

use super::{js_cell::JsCell, js_object::JsObject, js_value::JsValue};

pub struct JsFunction {}

pub struct JsBoundFunction {
    target: Handle<JsObject>,
    this_binding: JsValue,
    arguments: GcVec<JsValue>,
}

impl JsCell for JsBoundFunction {}
impl HeapObject for JsBoundFunction {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        self.target.visit_children(tracer);
        if self.this_binding.is_cell() && !self.this_binding.is_empty() {
            self.this_binding.as_cell_ref_mut().visit_children(tracer);
        }
        self.arguments.visit_children(tracer);
    }

    fn needs_destruction(&self) -> bool {
        false
    }
}

impl JsBoundFunction {
    pub fn this_binding(&self) -> JsValue {
        self.this_binding
    }
    pub fn target(&self) -> Handle<JsObject> {
        self.target
    }

    pub fn arguments(&self) -> &GcVec<JsValue> {
        &self.arguments
    }
}
