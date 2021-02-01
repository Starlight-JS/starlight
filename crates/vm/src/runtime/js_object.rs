use crate::{gc::heap_cell::HeapObject, heap::trace::Tracer};

use super::{
    indexed_elements::IndexedElements, js_cell::JsCell, js_value::JsValue, storage::FixedStorage,
};

pub type ObjectSlots = FixedStorage<JsValue>;
pub struct JsObject {
    slots: ObjectSlots,
    elements: IndexedElements,
    flags: u32,
}

pub const OBJ_FLAG_TUPLE: u32 = 0x4;
pub const OBJ_FLAG_CALLABLE: u32 = 0x2;
pub const OBJ_FLAG_EXTENSIBLE: u32 = 0x1;

impl HeapObject for JsObject {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        self.slots.data.visit_children(tracer);
        if self.elements.dense() {
            self.elements.vector.visit_children(tracer);
        }
        self.elements.map.visit_children(tracer);
    }
    fn needs_destruction(&self) -> bool {
        false
    }
}

impl JsCell for JsObject {}
