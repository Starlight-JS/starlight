/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{
    attributes::*,
    method_table::*,
    object::{EnumerationMode, JsObject, ObjectTag},
    slot::*,
    structure::Structure,
    symbol_table::{Internable, Symbol},
    value::*,
    Context,
};

use crate::gc::snapshot::deserializer::Deserializable;
use crate::gc::{
    cell::{GcCell, GcPointer, Trace},
    snapshot::serializer::{Serializable, SnapshotSerializer},
};
use crate::prelude::*;
use std::mem::size_of;

#[repr(C)]
pub struct JsString {
    pub string: String,
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.string.is_empty()
    }
    pub fn new(mut ctx: GcPointer<Context>, as_str: impl AsRef<str>) -> GcPointer<Self> {
        let str = as_str.as_ref();
        let proto = Self {
            string: str.to_owned(),
        };
        let cell = ctx.heap().allocate(proto);

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
}

pub struct JsStringObject {
    pub value: GcPointer<JsString>,
}

define_jsclass!(JsStringObject, String);

#[allow(non_snake_case)]
impl JsStringObject {
    pub fn new(ctx: GcPointer<Context>, s: GcPointer<JsString>) -> GcPointer<JsObject> {
        let obj = JsObject::new(
            ctx,
            &ctx.global_data().string_structure.unwrap(),
            Self::get_class(),
            ObjectTag::String,
        );
        unsafe {
            (obj.data::<Self>() as *mut _ as *mut Self).write(Self { value: s });
        }
        obj
    }

    pub fn new_plain(ctx: GcPointer<Context>, map: &GcPointer<Structure>) -> GcPointer<JsObject> {
        let obj = JsObject::new(ctx, map, Self::get_class(), ObjectTag::String);
        unsafe {
            (obj.data::<Self>() as *mut _ as *mut Self).write(Self {
                value: JsString::new(ctx, ""),
            });
        }
        obj
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        let value = obj.as_string_object().value;
        if index < value.len() {
            let ch = value.as_str().chars().nth(index as usize).unwrap();
            slot.set(
                JsValue::encode_object_value(JsString::new(ctx, ch.to_string())),
                string_indexed(),
            );
            return true;
        }
        JsObject::GetOwnIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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
        JsObject::GetOwnPropertyNamesMethod(obj, ctx, collector, mode)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let value = obj.as_string_object().value;
        if name == "length".intern() {
            slot.set(JsValue::new(value.len() as f64), string_length());
            return true;
        }
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }
}

impl Serializable for JsStringObject {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.value.serialize(serializer);
    }
}
