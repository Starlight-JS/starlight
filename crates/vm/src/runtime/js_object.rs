use crate::{gc::heap_cell::HeapObject, heap::trace::Tracer};

use super::{
    indexed_elements::IndexedElements, js_cell::JsCell, js_value::JsValue, storage::FixedStorage,
};

pub type ObjectSlots = FixedStorage<JsValue>;

#[repr(C)]
pub struct JsObject {
    slots: ObjectSlots,
    elements: IndexedElements,
    flags: u32,
    // We do not use Rust enums here as we do not want to allocate more than
    // needed memory for `ObjectData` type. If object's `tag` allows for non allocating
    // additional memory (i.e object is `Ordinary`) we just don't allocate additional memory.
    tag: ObjectTag,
    data: ObjectData,
}
#[repr(u8)]
pub enum ObjectTag {
    Ordinary,
    Array,
    Set,
    Map,
    Error,
    Global,
    Json,
    Function,
    Regex,
    ArrayBuffer,
    Int8Array,
    Uint8Array,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Int64Array,
    Uint64Array,
    Float32Array,
    Float64Array,
    Uint8ClampedArray,
    Reflect,
    Iterator,
    ArrayIterator,
    MapIterator,
    SetIterator,
    StringIterator,
    ForInIterator,
    WeakMap,
    WeakSet,

    NormalArguments,
    StrictArguments,

    Proxy,
}

#[repr(C)]
union ObjectData {
    ordinary: (),
    error: (),
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
