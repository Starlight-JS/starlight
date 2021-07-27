use wtf_rs::keep_on_stack;

use crate::constant::{S_REFERENCE_ERROR, S_SYNTAX_ERROR};
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::vm::attributes::*;
use crate::vm::class::JsClass;
use crate::vm::property_descriptor::DataDescriptor;
use crate::{
    constant::{S_ERROR, S_EVAL_ERROR, S_RANGE_ERROR, S_TYPE_ERROR, S_URI_ERROR},
    gc::cell::GcPointer,
    vm::{
        arguments::Arguments,
        builder::Builtin,
        context::Context,
        error::JsTypeError,
        error::*,
        function::JsNativeFunction,
        object::{JsObject, ObjectTag},
        string::JsString,
        structure::Structure,
        symbol_table::*,
        value::JsValue,
    },
};

pub fn error_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsError::new(ctx, msg, None)))
}

pub fn eval_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        ctx, msg, None,
    )))
}

pub fn reference_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsReferenceError::new(
        ctx, msg, None,
    )))
}

pub fn type_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )))
}

pub fn syntax_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        ctx, msg, None,
    )))
}

pub fn range_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsRangeError::new(
        ctx, msg, None,
    )))
}

pub fn uri_error_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsURIError::new(
        ctx, msg, None,
    )))
}

/// section 15.11.4.4 Error.prototype.toString()
pub fn error_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    let stack = ctx.shadowstack();
    if obj.is_jsobject() {
        letroot!(obj = stack, unsafe {
            obj.get_object().downcast_unchecked::<JsObject>()
        });
        let name;
        {
            let target = obj.get(ctx, "name")?;
            if target.is_undefined() {
                name = "UnknownError".to_owned();
            } else {
                name = target.to_string(ctx)?;
            }
        }
        let msg;
        {
            let target = obj.get(ctx, "message")?;
            if target.is_undefined() {
                msg = String::new();
            } else {
                msg = target.to_string(ctx)?;
            }
        }

        if name.is_empty() {
            return Ok(JsValue::encode_object_value(JsString::new(ctx, msg)));
        }
        if msg.is_empty() {
            return Ok(JsValue::encode_object_value(JsString::new(ctx, name)));
        }
        Ok(JsValue::encode_object_value(JsString::new(
            ctx,
            format!("{}: {}", name, msg),
        )))
    } else {
        let msg = JsString::new(ctx, "Base must be an object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }
}

impl Builtin for JsError {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data.object_prototype.unwrap();
        ctx.global_data.error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.eval_error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.range_error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.reference_error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.type_error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.syntax_error_structure = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data.uri_error_structure = Some(Structure::new_indexed(ctx, None, false));

        let structure = Structure::new_unique_with_proto(ctx, Some(obj_proto), false);
        let mut prototype = JsObject::new(ctx, &structure, JsError::class(), ObjectTag::Ordinary);
        ctx.global_data.error = Some(prototype);

        let mut constructor = JsNativeFunction::new(ctx, S_ERROR, error_constructor, 1);

        let name = JsString::new(ctx, S_ERROR);
        let message = JsString::new(ctx, "");

        def_native_property!(ctx, constructor, prototype, prototype, NONE)?;
        def_native_property!(ctx, prototype, constructor, constructor, W | C)?;
        def_native_property!(ctx, prototype, name, name, W | C)?;

        def_native_property!(ctx, prototype, message, message, W | C)?;
        def_native_method!(ctx, prototype, toString, error_to_string, 0, W | C)?;

        let mut global_object = ctx.global_object();
        def_native_property!(ctx, global_object, Error, constructor, W | C)?;

        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto =
                JsObject::new(ctx, &structure, JsEvalError::class(), ObjectTag::Ordinary);

            ctx.global_data
                .eval_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.eval_error = Some(sub_proto);

            let mut sub_ctor = JsNativeFunction::new(ctx, S_EVAL_ERROR, eval_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_EVAL_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;

            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            def_native_property!(ctx, global_object, EvalError, sub_ctor, W | C)?;
        }

        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto =
                JsObject::new(ctx, &structure, JsTypeError::class(), ObjectTag::Ordinary);

            keep_on_stack!(&structure, &mut sub_proto);

            ctx.global_data
                .type_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.type_error = Some(sub_proto);

            let mut sub_ctor = JsNativeFunction::new(ctx, S_TYPE_ERROR, type_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_TYPE_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;
            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            def_native_property!(ctx, global_object, TypeError, sub_ctor, W | C)?;
        }
        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto =
                JsObject::new(ctx, &structure, JsSyntaxError::class(), ObjectTag::Ordinary);

            keep_on_stack!(&structure, &mut sub_proto);

            ctx.global_data
                .syntax_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.syntax_error = Some(sub_proto);

            let mut sub_ctor =
                JsNativeFunction::new(ctx, S_SYNTAX_ERROR, syntax_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_SYNTAX_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;
            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            def_native_property!(ctx, global_object, SyntaxError, sub_ctor, W | C)?;
        }

        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto = JsObject::new(
                ctx,
                &structure,
                JsReferenceError::class(),
                ObjectTag::Ordinary,
            );

            ctx.global_data
                .reference_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.reference_error = Some(sub_proto);

            let mut sub_ctor =
                JsNativeFunction::new(ctx, S_REFERENCE_ERROR, reference_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_REFERENCE_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;

            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            let mut global_object = ctx.global_object();
            def_native_property!(ctx, global_object, ReferenceError, sub_ctor, W | C)?;
        }

        // range error
        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto = JsObject::new(
                ctx,
                &structure,
                JsReferenceError::class(),
                ObjectTag::Ordinary,
            );

            ctx.global_data
                .range_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.range_error = Some(sub_proto);

            let mut sub_ctor =
                JsNativeFunction::new(ctx, S_RANGE_ERROR, range_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_RANGE_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;
            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            let mut global_object = ctx.global_object();
            def_native_property!(ctx, global_object, RangeError, sub_proto, W | C)?;
        }

        {
            let structure = Structure::new_unique_with_proto(ctx, Some(prototype), false);
            let mut sub_proto =
                JsObject::new(ctx, &structure, JsURIError::class(), ObjectTag::Ordinary);

            ctx.global_data
                .uri_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            ctx.global_data.uri_error = Some(sub_proto);

            let mut sub_ctor = JsNativeFunction::new(ctx, S_URI_ERROR, uri_error_constructor, 1);

            def_native_property!(ctx, sub_ctor, prototype, sub_proto, NONE)?;
            def_native_property!(ctx, sub_proto, constructor, sub_ctor, W | C)?;

            let name = JsString::new(ctx, S_URI_ERROR);
            let message = JsString::new(ctx, "");

            def_native_property!(ctx, sub_proto, name, name, C)?;
            def_native_property!(ctx, sub_proto, message, message, W | C)?;
            def_native_method!(ctx, sub_proto, toString, error_to_string, 0, W | C)?;

            let mut global_object = ctx.global_object();
            def_native_property!(ctx, global_object, URIError, sub_proto, W | C)?;
        }

        Ok(())
    }
}
