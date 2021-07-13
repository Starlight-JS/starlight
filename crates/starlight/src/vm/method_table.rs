/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{Context, object::{EnumerationMode, JsHint, JsObject}, property_descriptor::PropertyDescriptor, slot::Slot, symbol_table::*, value::JsValue};
use crate::gc::cell::GcPointer;

pub type GetNonIndexedSlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    name: Symbol,
    slot: &mut Slot,
) -> Result<JsValue, JsValue>;

pub type GetIndexedSlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    index: u32,
    slot: &mut Slot,
) -> Result<JsValue, JsValue>;
pub type GetNonIndexedPropertySlotType =
    fn(obj: &mut GcPointer<JsObject>, ctx: &mut Context, name: Symbol, slot: &mut Slot) -> bool;
pub type GetIndexedPropertySlotType =
    fn(obj: &mut GcPointer<JsObject>, ctx: &mut Context, index: u32, slot: &mut Slot) -> bool;
pub type GetOwnNonIndexedPropertySlotType =
    fn(obj: &mut GcPointer<JsObject>, ctx: &mut Context, name: Symbol, slot: &mut Slot) -> bool;
pub type GetOwnIndexedPropertySlotType =
    fn(obj: &mut GcPointer<JsObject>, ctx: &mut Context, index: u32, slot: &mut Slot) -> bool;
pub type PutNonIndexedSlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    name: Symbol,
    val: JsValue,
    slot: &mut Slot,
    throwable: bool,
) -> Result<(), JsValue>;
pub type PutIndexedSlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    index: u32,
    val: JsValue,
    slot: &mut Slot,
    throwable: bool,
) -> Result<(), JsValue>;
pub type DeleteNonIndexedType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    name: Symbol,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type DeleteIndexedType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    index: u32,
    throwable: bool,
) -> Result<bool, JsValue>;

pub type DefineOwnNonIndexedPropertySlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    name: Symbol,
    desc: &PropertyDescriptor,
    slot: &mut Slot,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type DefineOwnIndexedPropertySlotType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    index: u32,
    desc: &PropertyDescriptor,
    slot: &mut Slot,
    throwable: bool,
) -> Result<bool, JsValue>;
pub type GetPropertyNamesType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    collector: &mut dyn FnMut(Symbol, u32),
    mode: EnumerationMode,
);
pub type GetOwnPropertyNamesType = fn(
    obj: &mut GcPointer<JsObject>,
    ctx: &mut Context,
    collector: &mut dyn FnMut(Symbol, u32),
    mode: EnumerationMode,
);

pub type DefaultValueType =
    fn(obj: &mut GcPointer<JsObject>, ctx: &mut Context, hint: JsHint) -> Result<JsValue, JsValue>;
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
