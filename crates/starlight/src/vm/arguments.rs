/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::mem::ManuallyDrop;

use super::{Context, environment::Environment, error::JsTypeError, method_table::*, object::{EnumerationMode, JsHint, JsObject, ObjectTag}, property_descriptor::*, slot::*, string::JsString, symbol_table::Internable, symbol_table::{Symbol, DUMMY_SYMBOL}, value::*};
use crate::gc::cell::{GcPointer, Trace, Tracer};
/// Arguments to JS function.
pub struct Arguments<'a> {
    /// 'this' value. In non-strict mode when this is undefined then global object is passed.
    pub this: JsValue,
    /// Argument values buffer.
    pub values: &'a mut [JsValue],
    /// Is current call is a constructor call?
    pub ctor_call: bool,
}

impl<'a> Arguments<'a> {
    #[deprecated = "Use [Arguments::new](Arguments::new) instead."]
    pub fn from_array_storage(_ctx: &mut Context, this: JsValue, values: &'a mut [JsValue]) -> Self {
        Self {
            this,
            values,
            ctor_call: false,
        }
    }
    /// Return count of passed arguments.
    pub fn size(&self) -> usize {
        self.values.len() as _
    }
    /// Create new argumens instance.
    pub fn new(this: JsValue, args: &'a mut [JsValue]) -> Self {
        Self {
            this,
            values: args,
            ctor_call: false,
        }
    }
    /// Get mutable reference to argument at `index`. If there's no such argument then
    /// this function panics with `"Out of bounds arguments"` message.
    pub fn at_mut(&mut self, x: usize) -> &mut JsValue {
        if x < self.size() {
            &mut self.values[x]
        } else {
            panic!("Out of bounds arguments");
        }
    }
    /// Get argument at `index`. If there's no such argument undefined is returned.
    pub fn at(&self, index: usize) -> JsValue {
        if index < self.size() {
            self.values[index]
        } else {
            JsValue::encode_undefined_value()
        }
    }
}

unsafe impl Trace for Arguments<'_> {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.this.trace(tracer);
        for value in self.values.iter_mut() {
            value.trace(tracer);
        }
    }
}

pub struct JsArguments {
    // TODO: Better alternative?
    pub mapping: Box<[Symbol]>,
    pub env: GcPointer<Environment>,
}
#[allow(non_snake_case)]
impl JsArguments {
    define_jsclass!(JsArguments, Arguments);
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, ctx, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, ctx, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        desc: &PropertyDescriptor,
        _slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        match obj.define_own_indexed_property_internal(ctx, index, desc, throwable) {
            Ok(false) | Err(_) => {
                if throwable {
                    let msg = JsString::new(ctx, "[[DefineOwnProperty]] failed");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }
                return Ok(false);
            }
            _ => {
                let arg = obj.as_arguments_mut();
                if arg.mapping.len() > index as usize {
                    let mapped = arg.mapping[index as usize];
                    if mapped != DUMMY_SYMBOL {
                        if desc.is_accessor() {
                            arg.mapping[index as usize] = DUMMY_SYMBOL;
                        } else if desc.is_data() {
                            let data = DataDescriptor { parent: *desc };
                            if !data.is_value_absent() {
                                arg.env.as_slice_mut()[mapped.get_index() as usize].value =
                                    desc.value();
                            }

                            if !data.is_writable_absent() && !data.is_writable() {
                                arg.mapping[index as usize] = DUMMY_SYMBOL;
                            }
                        }
                    }
                }

                Ok(true)
            }
        }
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if !JsObject::GetOwnIndexedPropertySlotMethod(obj, ctx, index, slot) {
            return false;
        }
        let arg = obj.as_arguments_mut();

        if arg.mapping.len() > index as usize {
            let mapped = arg.mapping[index as usize];
            if mapped != DUMMY_SYMBOL {
                let val = arg.env.as_slice()[mapped.get_index() as usize].value;
                let attrs = slot.attributes();
                slot.set(val, attrs);
            }
        }
        true
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, ctx, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, ctx, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, ctx, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, ctx, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, ctx, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let v = JsObject::GetNonIndexedSlotMethod(obj, ctx, name, slot);
        if name == "caller".intern() {
            match v {
                Ok(x) if x.is_callable() => {
                    if x.get_object()
                        .downcast::<JsObject>()
                        .unwrap()
                        .as_function()
                        .is_strict()
                    {
                        let msg =
                            JsString::new(ctx, "access to strict function 'caller' not allowed");
                        return Err(JsValue::encode_object_value(JsTypeError::new(
                            ctx, msg, None,
                        )));
                    }
                }
                _ => (),
            }
        }
        v
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        //!();
        JsObject::GetIndexedSlotMethod(obj, ctx, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, ctx, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
    pub fn new(
        ctx: &mut Context,
        env: GcPointer<Environment>,
        params: &[Symbol],
        len: u32,
        init: &[JsValue],
    ) -> GcPointer<JsObject> {
        letroot!(
            struct_ = ctx.shadowstack(),
            ctx.global_data().normal_arguments_structure.unwrap()
        );
        let mut obj = JsObject::new(
            ctx,
            &struct_,
            JsArguments::get_class(),
            ObjectTag::NormalArguments,
        );

        //let s = Structure::new_unique_indexed(ctx, None, true);

        let args = JsArguments {
            mapping: vec![].into_boxed_slice(),
            env,
        };
        *obj.data::<JsArguments>() = ManuallyDrop::new(args);
        use super::attributes::*;
        let mut mapping = Vec::with_capacity(params.len());
        for (i, param) in params.iter().enumerate().take(init.len()) {
            let mut slot = Slot::new();
            let _ = obj.define_own_indexed_property_slot(
                ctx,
                i as _,
                &*DataDescriptor::new(
                    init.get(i as usize)
                        .copied()
                        .unwrap_or_else(JsValue::encode_undefined_value),
                    create_data(AttrExternal::new(Some(W | C | E))).raw(),
                ),
                &mut slot,
                false,
            );

            mapping.push(*param);
        }
        let _ = obj.put(ctx, "length".intern(), JsValue::new(len as i32), false);
        obj.as_arguments_mut().mapping = mapping.into_boxed_slice();
        obj
    }
}

unsafe impl Trace for JsArguments {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.env.trace(tracer);
    }
}
