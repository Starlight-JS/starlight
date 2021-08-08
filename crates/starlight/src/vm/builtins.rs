/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! Module that contains builtin function definition. These functions is not exposed to JavaScript in any way.
//!
//!
//! Builtins is used most of the times as "slow path"s for regular operations (i.e call with spread parameter will invoke `reflect_apply`)
//!
//!

use super::{
    arguments::*, array::*, error::*, interpreter::frame::CallFrame, string::*, symbol_table::*,
};
use super::{value::*, Context};
use crate::gc::cell::GcPointer;
use crate::jsrt::get_length;
use crate::vm::class::JsClass;
pub unsafe fn reflect_apply(
    ctx: GcPointer<Context>,
    frame: &mut CallFrame,
    _ip: &mut *mut u8,
    _argc: u32,
    effect: u8,
) -> Result<(), JsValue> {
    let mut args = frame.pop();
    let mut func = frame.pop();
    let mut this = frame.pop();
    if !args.is_jsobject() {
        let msg = JsString::new(ctx, "expected array as arguments");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }
    let mut args = args.get_jsobject();
    if args.class as *const _ != JsArray::class() as *const _ {
        let msg = JsString::new(ctx, "not a callable object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    };
    let mut argsv = vec![];
    for i in 0..get_length(ctx, &mut args)? {
        argsv.push(args.get(ctx, Symbol::Index(i))?);
    }

    if !func.is_callable() {
        let msg = JsString::new(ctx, "not a callable object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }
    letroot!(func_object = gcstack, func.get_jsobject());
    letroot!(funcc = gcstack, func.get_jsobject());
    let func = func_object.as_function_mut();

    letroot!(args_ = gcstack, Arguments::new(this, &mut argsv));
    let result = if effect == 0 {
        func.call(ctx, &mut args_, JsValue::new(funcc))?
    } else {
        args_.ctor_call = true;
        func.construct(ctx, &mut args_, None, JsValue::new(funcc))?
    };
    frame.push(result);
    Ok(())
}

pub unsafe fn to_object(
    ctx: GcPointer<Context>,
    frame: &mut CallFrame,
    _ip: &mut *mut u8,
    _argc: u32,
    _effect: u8,
) -> Result<(), JsValue> {
    let value = frame.pop();
    let error_msg = frame.pop();

    if value.is_object() {
        frame.push(value);
    } else {
        let error_msg = error_msg.to_string(ctx)?;
        let msg = JsString::new(ctx, error_msg);
        return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
    }
    Ok(())
}

pub type Builtin =
    unsafe fn(GcPointer<Context>, &mut CallFrame, &mut *mut u8, u32, u8) -> Result<(), JsValue>;

pub static BUILTIN_FUNCS: [Builtin; 1] = [reflect_apply];

pub const BUILTIN_ARGS: [usize; 1] = [3];
