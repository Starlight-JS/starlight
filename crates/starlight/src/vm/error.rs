/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{Context, attributes::*, method_table::*, object::*, property_descriptor::*, string::JsString, structure::*, symbol_table::*, value::JsValue};
use crate::gc::cell::GcPointer;

pub struct JsError;
pub struct JsEvalError;
pub struct JsRangeError;
pub struct JsReferenceError;
pub struct JsSyntaxError;
pub struct JsTypeError;
pub struct JsURIError;
impl JsError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass!(JsObject, Error);
}

impl JsEvalError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().eval_error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass_with_symbol!(JsObject, Error, EvalError);
}

impl JsRangeError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().range_error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass_with_symbol!(JsObject, Error, RangeError);
}

impl JsReferenceError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().reference_error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass_with_symbol!(JsObject, Error, ReferenceError);
}

impl JsSyntaxError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().syntax_error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass_with_symbol!(JsObject, Error, SyntaxError);
}

impl JsTypeError {
    pub fn new(
        ctx: &mut Context,
        s: GcPointer<JsString>,
        structure: Option<GcPointer<Structure>>,
    ) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            structure.unwrap_or_else(|| ctx.global_data().type_error_structure.unwrap())
        );
        let mut obj = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Ordinary);
        let stack = ctx.stacktrace();
        let str = JsString::new(ctx, stack);
        let _ = obj.define_own_property(
            ctx,
            "stack".intern(),
            &*DataDescriptor::new(JsValue::new(str), W | C),
            false,
        );
        if !s.as_str().is_empty() {
            let _ = obj.define_own_property(
                ctx,
                "message".intern(),
                &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
                false,
            );
        }

        obj
    }
    define_jsclass_with_symbol!(JsObject, Error, TypeError);
}
