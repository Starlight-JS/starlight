/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::intrinsics::unlikely;

use super::{
    attributes::*,
    class::JsClass,
    error::{JsRangeError, JsTypeError},
    indexed_elements::MAX_VECTOR_SIZE,
    object::*,
    string::JsString,
    Context,
};
use super::{
    method_table::*,
    object::EnumerationMode,
    symbol_table::{Internable, Symbol},
};
use super::{property_descriptor::PropertyDescriptor, slot::*, value::*};
use crate::gc::cell::GcPointer;
pub struct JsArray;
#[allow(non_snake_case)]
impl JsArray {
    pub fn from_slice(ctx: GcPointer<Context>, slice: &[JsValue]) -> GcPointer<JsObject> {
        let mut this = Self::new(ctx, slice.len() as _);

        for i in 0..slice.len() {
            let val = slice[i];
            let _ = this.put(ctx, Symbol::Index(i as _), val, false);
        }

        this
    }
    pub fn new(ctx: GcPointer<Context>, n: u32) -> GcPointer<JsObject> {
        let mut arr = JsObject::new(
            ctx,
            &ctx.global_data().array_structure.unwrap(),
            Self::get_class(),
            ObjectTag::Array,
        );
        arr.indexed.set_length(n);
        arr
    }
    define_jsclass!(JsArray, Array);
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, ctx, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, ctx, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, ctx, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, ctx, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, ctx, name, val, slot, throwable)
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
        JsObject::GetOwnPropertyNamesMethod(obj, ctx, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == "length".intern() {
            if throwable {
                let msg = JsString::new(ctx, "delete failed");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }
        JsObject::DeleteNonIndexedMethod(obj, ctx, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, ctx, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, ctx, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, ctx, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if name == "length".intern() {
            slot.set_1(
                JsValue::new(obj.indexed.length() as f64),
                if obj.indexed.writable() {
                    create_data(AttrExternal::new(Some(W)))
                } else {
                    create_data(AttrExternal::new(Some(N)))
                },
                Some(obj.as_dyn()),
            );
            return true;
        }
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == "length".intern() {
            return obj.define_length_property(ctx, desc, throwable);
        }
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, ctx, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
}

impl GcPointer<JsObject> {
    fn change_length_writable(
        &mut self,
        ctx: GcPointer<Context>,
        writable: bool,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if !writable {
            self.indexed.make_readonly();
        } else if !self.indexed.writable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing [[Writable]] of unconfigurable property not allowed",
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }
        Ok(true)
    }

    fn define_length_property(
        &mut self,
        ctx: GcPointer<Context>,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if desc.is_configurable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing [[Configurable]] of unconfigurable property not allowed",
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }

        if desc.is_enumerable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing [[Enumerable]] of unconfigurable property not allowed",
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }

        if desc.is_accessor() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing description of unconfigurable property not allowed",
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }

            return Ok(false);
        }

        if desc.is_value_absent() {
            if !desc.is_writable_absent() {
                return self.change_length_writable(ctx, desc.is_writable(), throwable);
            }
            return Ok(true);
        }

        let new_len_double = desc.value().to_number(ctx)?;
        let new_len = new_len_double as u32;
        if new_len as f64 != new_len_double {
            let msg = JsString::new(ctx, "invalid array length");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }

        let old_len = self.indexed.length();
        if new_len == old_len {
            if !desc.is_writable_absent() {
                return self.change_length_writable(ctx, desc.is_writable(), throwable);
            }
            return Ok(true);
        }

        if !self.indexed.writable() {
            if throwable {
                let msg = JsString::new(ctx, "'length' not writable");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }
        let succ = self.set_length(ctx, new_len, throwable)?;
        if !desc.is_writable_absent() {
            return self.change_length_writable(ctx, desc.is_writable(), throwable);
        }
        Ok(succ)
    }

    fn set_length(
        &mut self,
        mut ctx: GcPointer<Context>,
        len: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if unlikely(len == 4294967295) {
            let msg = JsString::new(ctx, "Out of memory for array values");
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }
        let mut old = self.indexed.length();
        if len >= old {
            self.indexed.set_length(len);
            return Ok(true);
        }

        // dense array shrink
        if self.indexed.dense() {
            if len > MAX_VECTOR_SIZE as u32 {
                if let Some(map) = self.indexed.map.as_mut() {
                    let mut copy = vec![];
                    map.iter().for_each(|x| {
                        copy.push(*x.0);
                    });
                    copy.sort_unstable();
                    for x in copy.iter() {
                        if *x >= len {
                            map.remove(x);
                        } else {
                            break;
                        }
                    }

                    if map.is_empty() {
                        self.indexed.make_dense();
                    }
                }
            } else {
                self.indexed.make_dense();
                if self.indexed.vector.size() > len {
                    let stack = ctx.shadowstack();
                    letroot!(vector = stack, self.indexed.vector);
                    vector.mut_handle().resize(ctx.heap(), len as _);
                }
            }
            self.indexed.set_length(len);
            return Ok(true);
        }
        if (old - len) < (1 << 24) {
            while len < old {
                old -= 1;
                if !self.delete_indexed_internal(ctx, old, false)? {
                    self.indexed.set_length(old + 1);
                    if throwable {
                        let msg = JsString::new(ctx, "failed to shrink array");
                        return Err(JsValue::encode_object_value(JsTypeError::new(
                            ctx, msg, None,
                        )));
                    }
                    return Ok(false);
                }
            }
            self.indexed.set_length(len);
            return Ok(true);
        }

        let mut props = Vec::new();
        self.get_own_property_names(
            ctx,
            &mut |sym, off| {
                props.push((sym, off));
            },
            EnumerationMode::IncludeNotEnumerable,
        );

        for it in props.iter().rev() {
            let sym = it.0;
            match sym {
                Symbol::Index(index) => {
                    if !self.delete_indexed_internal(ctx, index, false)? {
                        self.indexed.set_length(index + 1);
                        if throwable {
                            let msg = JsString::new(ctx, "failed to shrink array");
                            return Err(JsValue::encode_object_value(JsTypeError::new(
                                ctx, msg, None,
                            )));
                        }
                        return Ok(false);
                    }
                }
                _ => continue,
            }
        }
        self.indexed.set_length(len);

        Ok(true)
    }
}

impl JsClass for JsArray {
    fn class() -> &'static super::class::Class {
        Self::get_class()
    }
}
