/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{vm::{context::Context, arguments::Arguments, error::JsTypeError, error::*, object::JsObject, slot::*, string::JsString, symbol_table::*, value::JsValue}};

pub fn error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsError::new(ctx, msg, None)))
}

pub fn eval_error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        ctx, msg, None,
    )))
}

pub fn reference_error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsReferenceError::new(
        ctx, msg, None,
    )))
}

pub fn type_error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )))
}

pub fn syntax_error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        ctx, msg, None,
    )))
}

pub fn range_error_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(ctx)?;
    let msg = JsString::new(ctx, message);
    Ok(JsValue::encode_object_value(JsRangeError::new(
        ctx, msg, None,
    )))
}

/// section 15.11.4.4 Error.prototype.toString()
pub fn error_to_string(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    let stack = ctx.shadowstack();
    if obj.is_jsobject() {
        letroot!(obj = stack, unsafe {
            obj.get_object().downcast_unchecked::<JsObject>()
        });
        let name;
        {
            let mut slot = Slot::new();
            let target = obj.get_slot(ctx, "name".intern(), &mut slot)?;
            if target.is_undefined() {
                name = "UnknownError".to_owned();
            } else {
                name = target.to_string(ctx)?;
            }
        }
        let msg;
        {
            let target = obj.get(ctx, "message".intern())?;
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
