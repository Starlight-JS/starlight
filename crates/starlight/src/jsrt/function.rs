/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    bytecompiler::*,
    letroot,
    vm::Runtime,
    vm::{
        arguments::Arguments,
        array_storage::ArrayStorage,
        error::JsTypeError,
        function::*,
        slot::*,
        string::JsString,
        symbol_table::{Internable, Symbol},
        value::JsValue,
    },
};

pub fn function_to_string(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    let obj = &args.this;
    if obj.is_callable() {
        letroot!(func = stack, obj.to_object(vm)?);
        let mut slot = Slot::new();
        let mut fmt = "function ".to_string();
        if func.get_own_property_slot(vm, "name".intern(), &mut slot) {
            let name = slot.get(vm, *obj)?;
            let name_str = name.to_string(vm)?;
            if name_str.is_empty() {
                fmt.push_str("<anonymous>");
            } else {
                fmt.push_str(&name_str);
            }
        } else {
            fmt.push_str("<anonymous>");
        }

        fmt.push_str("() { [native code] }");
        return Ok(JsValue::encode_object_value(JsString::new(vm, fmt)));
    }
    let msg = JsString::new(vm, "Function.prototype.toString is not generic");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        vm, msg, None,
    )));
}

pub fn function_prototype(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut params = vec![];
    if args.size() >= 2 {
        for i in 0..args.size() - 1 {
            params.push(args.at(i).to_string(vm)?);
        }
    }
    let body = if args.size() == 0 {
        "{ }".to_owned()
    } else {
        format!("{{ {} }}", args.at(args.size() - 1).to_string(vm)?)
    };
    let rel_path = unsafe { (*vm.stack.current).code_block.unwrap().path.clone() };
    ByteCompiler::compile_code(
        vm, &params, &rel_path, body, false,
    )
}

pub fn function_bind(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    letroot!(obj = stack, args.this);

    if obj.is_callable() {
        letroot!(
            vals = stack,
            ArrayStorage::with_size(
                vm,
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
            vm,
            FuncType::Bound(JsBoundFunction {
                args: *vals,
                this: args.at(0),
                target: unsafe { obj.get_object().downcast_unchecked() },
            }),
            false,
        );

        return Ok(JsValue::encode_object_value(f));
    }
    let msg = JsString::new(vm, "Function.prototype.bind is not generic");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        vm, msg, None,
    )));
}

pub fn function_apply(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    letroot!(this = stack, args.this);
    if this.is_callable() {
        letroot!(obj = stack, this.get_jsobject());
        letroot!(objc = stack, *obj);
        let func = obj.as_function_mut();

        let args_size = args.size();
        let arg_array = args.at(1);
        if args_size == 1 || arg_array.is_null() || arg_array.is_undefined() {
            letroot!(args = stack, Arguments::new(args.at(0), &mut []));
            return func.call(rt, &mut args, JsValue::new(*objc));
        }

        if !arg_array.is_jsobject() {
            let msg = JsString::new(
                rt,
                "Function.prototype.apply requires array-like as 2nd argument",
            );
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }

        letroot!(arg_array = stack, arg_array.get_jsobject());
        let len = super::get_length(rt, &mut arg_array)?;
        let mut argsv = Vec::with_capacity(len as usize);

        for i in 0..len {
            argsv.push(arg_array.get(rt, Symbol::Index(i))?);
        }
        crate::letroot!(args_ = stack, Arguments::new(args.at(0), &mut argsv));
        return func.call(rt, &mut args_, JsValue::new(*objc));
    }

    let msg = JsString::new(rt, "Function.prototype.apply is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        rt, msg, None,
    )))
}

pub fn function_call(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this;
    let stack = rt.shadowstack();
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

        return func.call(rt, &mut args_, JsValue::new(*objc));
    }

    let msg = JsString::new(rt, "Function.prototype.call is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        rt, msg, None,
    )))
}
