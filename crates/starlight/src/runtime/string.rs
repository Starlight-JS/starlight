use super::{attributes::*, slot::*};
use super::{
    method_table::MethodTable, object::EnumerationMode, property_descriptor::PropertyDescriptor,
};
use super::{object::JsHint, symbol::*};
use super::{
    object::{JsObject, ObjectTag},
    value::JsValue,
};
use crate::{
    gc::cell::{Cell, Gc, Trace},
    vm::VirtualMachine,
};

#[repr(C)]
pub struct JsString {
    str: String,
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.str.is_empty()
    }
    pub fn new(vm: &mut VirtualMachine, as_str: impl AsRef<str>) -> Gc<Self> {
        let str = as_str.as_ref();
        let proto = Self {
           str:str.to_string()
            // len: str.len() as _,
            //data: [],
        };
        let mut cell = vm.space().alloc(proto);

        /*unsafe {
            cell.len = str.len() as _;
            std::ptr::copy_nonoverlapping(
                str.as_bytes().as_ptr(),
                cell.data.as_mut_ptr(),
                str.len(),
            );
        }*/

        cell
    }

    pub fn as_str(&self) -> &str {
        &self.str
        /*unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                self.data.as_ptr(),
                self.len as _,
            ))
        }*/
    }

    pub fn len(&self) -> u32 {
        self.str.len() as _
    }
}

impl Cell for JsString {}
unsafe impl Trace for JsString {}

pub struct JsStringObject {
    value: Gc<JsString>,
}

#[allow(non_snake_case)]
impl JsStringObject {
    pub fn value(&self) -> Gc<JsString> {
        self.value
    }

    pub fn new(vm: &mut VirtualMachine, value: Gc<JsString>) -> Gc<JsObject> {
        let mut jsobject = JsObject::new(
            vm,
            vm.global_data().string_structure.unwrap(),
            Self::get_class(),
            ObjectTag::String,
        );

        jsobject
    }

    define_jsclass!(JsStringObject, String);
    pub fn GetPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        let value = obj.as_string().value();
        if index < value.len() {
            let nstr = JsString::new(
                vm,
                value
                    .as_str()
                    .chars()
                    .nth(index as usize)
                    .unwrap()
                    .to_string(),
            );
            slot.set_1(JsValue::new(nstr), string_indexed(), Some(obj.as_dyn()));
            return true;
        }
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        if mode == EnumerationMode::IncludeNotEnumerable {
            collector(Symbol::length(), 0);
        }
        let value = obj.as_string();
        for x in 0..obj.as_string().value().as_str().len() {
            collector(Symbol::Indexed(x as _), x as u32);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == Symbol::length() {
            if throwable {
                todo!();
            }
            return Ok(false);
        }
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if name == Symbol::length() {
            slot.set_1(
                JsValue::new(obj.as_string().value.as_str().len() as i32),
                string_length(),
                Some(obj.as_dyn()),
            );
            return true;
        }
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
}
