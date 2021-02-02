use std::mem::size_of;

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

use super::{
    indexed_elements::IndexedElements, js_cell::JsCell, js_function::JsFunction, js_value::JsValue,
    storage::FixedStorage, structure::Structure,
};

pub type ObjectSlots = FixedStorage<JsValue>;

#[repr(C)]
pub struct JsObject {
    structure: Handle<Structure>,
    slots: ObjectSlots,
    elements: IndexedElements,
    flags: u32,
    // We do not use Rust enums here as we do not want to allocate more than
    // needed memory for `ObjectData` type. If object's `tag` allows for non allocating
    // additional memory (i.e object is `Ordinary`) we just don't allocate additional memory.
    tag: ObjectTag,
    data: ObjectData,
}
impl JsObject {
    pub fn is_function(&self) -> bool {
        self.tag == ObjectTag::Function
    }

    pub fn get_function(&self) -> &JsFunction {
        assert!(self.is_function());
        unsafe { &self.data.function }
    }

    pub fn get_function_mut(&mut self) -> &mut JsFunction {
        assert!(self.is_function());
        unsafe { &mut self.data.function }
    }
}
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
pub fn object_size_for_tag(tag: ObjectTag) -> usize {
    let size = size_of::<JsObject>() - size_of::<ObjectData>();
    match tag {
        ObjectTag::Function => size_of::<JsFunction>() + size,
        _ => size,
    }
}
#[repr(C)]
union ObjectData {
    ordinary: (),
    function: JsFunction,
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
