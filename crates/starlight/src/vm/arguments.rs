/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::mem::ManuallyDrop;

use crate::prelude::*;

use super::{
    environment::Environment,
    error::JsTypeError,
    method_table::*,
    object::{JsObject, ObjectTag},
    property_descriptor::*,
    slot::*,
    string::JsString,
    symbol_table::Internable,
    symbol_table::{Symbol, DUMMY_SYMBOL},
    value::*,
    Context,
};
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
    pub fn from_array_storage(
        _ctx: GcPointer<Context>,
        this: JsValue,
        values: &'a mut [JsValue],
    ) -> Self {
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

    pub fn try_at(&self, index: usize) -> Option<JsValue> {
        if index < self.size() {
            Some(self.values[index])
        } else {
            None
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

impl JsClass for JsArguments {
    fn class() -> &'static Class {
        define_jsclass!(JsArguments, Arguments)
    }
}

#[allow(non_snake_case)]
impl JsArguments {
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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
        ctx: GcPointer<Context>,
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

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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
    pub fn new(
        ctx: GcPointer<Context>,
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
            JsArguments::class(),
            ObjectTag::NormalArguments,
        );

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
