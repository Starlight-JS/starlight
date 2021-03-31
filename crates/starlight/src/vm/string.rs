use super::{
    attributes::*,
    method_table::*,
    object::{EnumerationMode, JsHint, JsObject, ObjectTag},
    property_descriptor::PropertyDescriptor,
    slot::*,
    structure::Structure,
    symbol_table::{Internable, Symbol},
    value::*,
    Runtime,
};
use crate::gc::snapshot::deserializer::Deserializable;
use crate::gc::{
    cell::{GcCell, GcPointer, Trace},
    snapshot::serializer::{Serializable, SnapshotSerializer},
};
use std::mem::size_of;

#[repr(C)]
pub struct JsString {
    pub(crate) string: String,
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.string.is_empty()
    }
    pub fn new(vm: &mut Runtime, as_str: impl AsRef<str>) -> GcPointer<Self> {
        let str = as_str.as_ref();
        let proto = Self {
            string: str.to_owned(),
        };
        let cell = vm.heap().allocate(proto);

        cell
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }

    pub fn len(&self) -> u32 {
        self.string.len() as _
    }
}

unsafe impl Trace for JsString {}
impl GcCell for JsString {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    fn compute_size(&self) -> usize {
        size_of::<Self>()
    }
    vtable_impl!();
}

pub struct JsStringObject {
    pub value: GcPointer<JsString>,
}

#[allow(non_snake_case)]
impl JsStringObject {
    pub fn new(rt: &mut Runtime, s: GcPointer<JsString>) -> GcPointer<JsObject> {
        let obj = JsObject::new(
            rt,
            &rt.global_data().string_structure.unwrap(),
            Self::get_class(),
            ObjectTag::String,
        );
        unsafe {
            (obj.data::<Self>() as *mut _ as *mut Self).write(Self { value: s });
        }
        obj
    }

    pub fn new_plain(rt: &mut Runtime, map: &GcPointer<Structure>) -> GcPointer<JsObject> {
        let obj = JsObject::new(rt, map, Self::get_class(), ObjectTag::String);
        unsafe {
            (obj.data::<Self>() as *mut _ as *mut Self).write(Self {
                value: JsString::new(rt, ""),
            });
        }
        obj
    }
    define_jsclass!(JsStringObject, String);
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        let value = obj.as_string_object().value;
        if index < value.len() {
            let ch = value.as_str().chars().nth(index as usize).unwrap();
            slot.set(
                JsValue::encode_object_value(JsString::new(vm, ch.to_string())),
                string_indexed(),
            );
            return true;
        }
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        if mode == EnumerationMode::IncludeNotEnumerable {
            collector("length".intern(), 0);
        }
        let value = obj.as_string_object().value;
        for i in 0..value.len() {
            collector(Symbol::Index(i), i);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let value = obj.as_string_object().value;
        if name == "length".intern() {
            slot.set(
                JsValue::encode_f64_value(value.len() as f64),
                string_length(),
            );
            return true;
        }
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
}

impl Serializable for JsStringObject {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.value.serialize(serializer);
    }
}
