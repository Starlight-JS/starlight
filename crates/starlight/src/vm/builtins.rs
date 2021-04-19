//! Module that contains builtin function definition. These functions is not exposed to JavaScript in any way.
//!
//!
//! Builtins is used most of the times as "slow path"s for regular operations (i.e call with spread parameter will invoke `reflect_apply`)
//!
//!

use super::value::*;
use super::{
    arguments::*, array::*, error::*, interpreter::frame::CallFrame, string::*, symbol_table::*,
    Runtime,
};
use crate::jsrt::get_length;
pub unsafe fn reflect_apply(
    rt: &mut Runtime,
    frame: &mut CallFrame,
    _ip: &mut *mut u8,
    _argc: u32,
    effect: u8,
) -> Result<(), JsValue> {
    let gcstack = rt.shadowstack();

    let mut args = frame.pop();
    let mut func = frame.pop();
    let mut this = frame.pop();
    if !args.is_jsobject() {
        let msg = JsString::new(rt, "expected array as arguments");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    }
    letroot!(args = gcstack, args.get_jsobject());
    if args.class() as *const _ != JsArray::get_class() as *const _ {
        let msg = JsString::new(rt, "not a callable object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    };
    let mut argsv = vec![];
    for i in 0..get_length(rt, &mut args)? {
        argsv.push(args.get(rt, Symbol::Index(i))?);
    }

    if !func.is_callable() {
        let msg = JsString::new(rt, "not a callable object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    }
    letroot!(func_object = gcstack, func.get_jsobject());
    letroot!(funcc = gcstack, func.get_jsobject());
    let func = func_object.as_function_mut();

    letroot!(args_ = gcstack, Arguments::new(this, &mut argsv));
    let result = if effect == 0 {
        func.call(rt, &mut args_, JsValue::new(*funcc))?
    } else {
        args_.ctor_call = true;
        func.construct(rt, &mut args_, None, JsValue::new(*funcc))?
    };
    frame.push(result);
    Ok(())
}

pub unsafe fn to_object(
    rt: &mut Runtime,
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
        let error_msg = error_msg.to_string(rt)?;
        let msg = JsString::new(rt, error_msg);
        return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
    }
    Ok(())
}

pub type Builtin =
    unsafe fn(&mut Runtime, &mut CallFrame, &mut *mut u8, u32, u8) -> Result<(), JsValue>;

pub static BUILTINS: [Builtin; 1] = [reflect_apply];
