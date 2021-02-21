use super::slot::*;
use super::symbol::*;
use super::value::*;
use super::{object::*, property_descriptor::PropertyDescriptor};
use crate::gc::cell::*;
use crate::vm::*;
pub type GetNonIndexedSlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    name: Symbol,
    slot: &mut Slot,
) -> Result<JsValue, JsValue>;

pub type GetIndexedSlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    index: u32,
    slot: &mut Slot,
) -> Result<JsValue, JsValue>;
pub type GetNonIndexedPropertySlotType =
    fn(obj: Gc<JsObject>, vm: &mut VirtualMachine, name: Symbol, slot: &mut Slot) -> bool;
pub type GetIndexedPropertySlotType =
    fn(obj: Gc<JsObject>, vm: &mut VirtualMachine, index: u32, slot: &mut Slot) -> bool;
pub type GetOwnNonIndexedPropertySlotType =
    fn(obj: Gc<JsObject>, vm: &mut VirtualMachine, name: Symbol, slot: &mut Slot) -> bool;
pub type GetOwnIndexedPropertySlotType =
    fn(obj: Gc<JsObject>, vm: &mut VirtualMachine, index: u32, slot: &mut Slot) -> bool;
pub type PutNonIndexedSlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    name: Symbol,
    val: JsValue,
    slot: &mut Slot,
    throwable: bool,
) -> Result<(), JsValue>;
pub type PutIndexedSlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    index: u32,
    val: JsValue,
    slot: &mut Slot,
    throwable: bool,
) -> Result<(), JsValue>;
pub type DeleteNonIndexedType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    name: Symbol,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type DeleteIndexedType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    index: u32,
    throwable: bool,
) -> Result<bool, JsValue>;

pub type DefineOwnNonIndexedPropertySlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    name: Symbol,
    desc: &PropertyDescriptor,
    slot: &mut Slot,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type DefineOwnIndexedPropertySlotType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    index: u32,
    desc: &PropertyDescriptor,
    slot: &mut Slot,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type GetPropertyNamesType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    collector: &mut dyn FnMut(Symbol, u32),
    mode: EnumerationMode,
);
pub type GetOwnPropertyNamesType = fn(
    obj: Gc<JsObject>,
    vm: &mut VirtualMachine,
    collector: &mut dyn FnMut(Symbol, u32),
    mode: EnumerationMode,
);

pub type DefaultValueType =
    fn(obj: Gc<JsObject>, vm: &mut VirtualMachine, hint: JsHint) -> Result<JsValue, JsValue>;
#[derive(Clone, Copy)]
#[repr(C)]
#[allow(non_snake_case)]
pub struct MethodTable {
    pub GetNonIndexedSlot: GetNonIndexedSlotType,
    pub GetIndexedSlot: GetIndexedSlotType,
    pub GetNonIndexedPropertySlot: GetNonIndexedPropertySlotType,
    pub GetIndexedPropertySlot: GetIndexedPropertySlotType,
    pub GetOwnNonIndexedPropertySlot: GetOwnNonIndexedPropertySlotType,
    pub GetOwnIndexedPropertySlot: GetOwnIndexedPropertySlotType,
    pub PutNonIndexedSlot: PutNonIndexedSlotType,
    pub PutIndexedSlot: PutIndexedSlotType,
    pub DeleteNonIndexed: DeleteNonIndexedType,
    pub DeleteIndexed: DeleteIndexedType,
    pub DefineOwnNonIndexedPropertySlot: DefineOwnNonIndexedPropertySlotType,
    pub DefineOwnIndexedPropertySlot: DefineOwnIndexedPropertySlotType,
    pub GetPropertyNames: GetPropertyNamesType,
    pub GetOwnPropertyNames: GetOwnPropertyNamesType,
    pub DefaultValue: DefaultValueType,
}

#[macro_export]
macro_rules! js_method_table {
    ($class: ident) => {
        MethodTable {
            GetNonIndexedSlot: $class::GetNonIndexedSlotMethod,
            GetIndexedSlot: $class::GetIndexedSlotMethod,
            GetNonIndexedPropertySlot: $class::GetNonIndexedPropertySlotMethod,
            GetIndexedPropertySlot: $class::GetIndexedPropertySlotMethod,
            GetOwnNonIndexedPropertySlot: $class::GetOwnNonIndexedPropertySlotMethod,
            GetOwnIndexedPropertySlot: $class::GetOwnIndexedPropertySlotMethod,
            PutNonIndexedSlot: $class::PutNonIndexedSlotMethod,
            PutIndexedSlot: $class::PutIndexedSlotMethod,
            DeleteNonIndexed: $class::DeleteNonIndexedMethod,
            DeleteIndexed: $class::DeleteIndexedMethod,
            DefineOwnNonIndexedPropertySlot: $class::DefineOwnNonIndexedPropertySlotMethod,
            DefineOwnIndexedPropertySlot: $class::DefineOwnIndexedPropertySlotMethod,
            GetPropertyNames: $class::GetPropertyNamesMethod,
            GetOwnPropertyNames: $class::GetOwnPropertyNamesMethod,
            DefaultValue: $class::DefaultValueMethod,
        }
    };
}
