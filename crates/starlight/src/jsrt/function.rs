/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{bytecompiler::*, letroot, vm::{Context}, vm::{
        arguments::Arguments,
        array_storage::ArrayStorage,
        error::JsTypeError,
        function::*,
        slot::*,
        string::JsString,
        symbol_table::{Internable, Symbol},
        value::JsValue,
    }};

pub fn function_to_string(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    let obj = &args.this;
    if obj.is_callable() {
        letroot!(func = stack, obj.to_object(ctx)?);
        let mut slot = Slot::new();
        let mut fmt = "function ".to_string();
        if func.get_own_property_slot(ctx, "name".intern(), &mut slot) {
            let name = slot.get(ctx, *obj)?;
            let name_str = name.to_string(ctx)?;
            if name_str.is_empty() {
                fmt.push_str("<anonymous>");
            } else {
                fmt.push_str(&name_str);
            }
        } else {
            fmt.push_str("<anonymous>");
        }

        fmt.push_str("() { [native code] }");
        return Ok(JsValue::encode_object_value(JsString::new(ctx, fmt)));
    }
    let msg = JsString::new(ctx, "Function.prototype.toString is not generic");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )));
}

pub fn function_prototype(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut params = vec![];
    if args.size() >= 2 {
        for i in 0..args.size() - 1 {
            params.push(args.at(i).to_string(ctx)?);
        }
    }
    let body = if args.size() == 0 {
        "{ }".to_owned()
    } else {
        format!("{{ {} }}", args.at(args.size() - 1).to_string(ctx)?)
    };
    let rel_path = unsafe { (*ctx.stack.current).code_block.unwrap().path.clone() };
    ByteCompiler::compile_code(ctx, &params, &rel_path, body, false).map_err(|e| JsValue::from(ctx.new_syntax_error(format!("Compile Error {:?}",e))))
}

pub fn function_bind(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(obj = stack, args.this);

    if obj.is_callable() {
        letroot!(
            vals = stack,
            ArrayStorage::with_size(
                ctx,
                if args.size() == 0 {
                    0
                } else {
                    args.size() as u32 - 1
                },
                if args.size() == 0 {
                    0
                } else {
                    args.size() as u32 - 1
                },
            )
        );
        for i in 1..args.size() as u32 {
            *vals.at_mut(i - 1) = args.at(i as _);
        }
        let f = JsFunction::new(
            ctx,
            FuncType::Bound(JsBoundFunction {
                args: *vals,
                this: args.at(0),
                target: unsafe { obj.get_object().downcast_unchecked() },
            }),
            false,
        );

        return Ok(JsValue::encode_object_value(f));
    }
    let msg = JsString::new(ctx, "Function.prototype.bind is not generic");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )));
}

pub fn function_apply(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(this = stack, args.this);
    if this.is_callable() {
        letroot!(obj = stack, this.get_jsobject());
        letroot!(objc = stack, *obj);
        let func = obj.as_function_mut();

        let args_size = args.size();
        let arg_array = args.at(1);
        if args_size == 1 || arg_array.is_null() || arg_array.is_undefined() {
            letroot!(args = stack, Arguments::new(args.at(0), &mut []));
            return func.call(ctx, &mut args, JsValue::new(*objc));
        }

        if !arg_array.is_jsobject() {
            let msg = JsString::new(
                ctx,
                "Function.prototype.apply requires array-like as 2nd argument",
            );
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }

        letroot!(arg_array = stack, arg_array.get_jsobject());
        let len = super::get_length(ctx, &mut arg_array)?;
        let mut argsv = Vec::with_capacity(len as usize);

        for i in 0..len {
            argsv.push(arg_array.get(ctx, Symbol::Index(i))?);
        }
        crate::letroot!(args_ = stack, Arguments::new(args.at(0), &mut argsv));
        return func.call(ctx, &mut args_, JsValue::new(*objc));
    }

    let msg = JsString::new(ctx, "Function.prototype.apply is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )))
}

pub fn function_call(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this;
    let stack = ctx.shadowstack();
    if this.is_callable() {
        letroot!(obj = stack, this.get_jsobject());
        letroot!(objc = stack, *obj);
        let func = obj.as_function_mut();

        let args_size = args.size();
        let mut argsv = vec![];
        if args_size > 1 {
            for i in 0..args_size - 1 {
                argsv.push(args.at(i + 1));
                //*args_.at_mut(i) = args.at(i + 1);
            }
        }
        letroot!(args_ = stack, Arguments::new(args.at(0), &mut argsv,));

        return func.call(ctx, &mut args_, JsValue::new(*objc));
    }

    let msg = JsString::new(ctx, "Function.prototype.call is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )))
}
