use crate::{
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
    let obj = args.this;
    if obj.is_callable() {
        let func = obj.to_object(vm)?;
        let mut slot = Slot::new();
        let mut fmt = "function ".to_string();
        if func.get_own_property_slot(vm, "name".intern(), &mut slot) {
            let name = slot.get(vm, obj)?;
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
    let _ = vm;
    let _ = args;
    Ok(JsValue::encode_undefined_value())
}

pub fn function_bind(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;

    if obj.is_callable() {
        let mut vals = ArrayStorage::with_size(
            vm.heap(),
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
        );
        for i in 1..args.size() as u32 {
            *vals.at_mut(i - 1) = args.at(i as _);
        }
        let f = JsFunction::new(
            vm,
            FuncType::Bound(JsBoundFunction {
                args: vals,
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
    let this = args.this;
    if this.is_callable() {
        let mut obj = this.get_jsobject();
        let func = obj.as_function_mut();

        let args_size = args.size();
        let arg_array = args.at(1);
        if args_size == 1 || arg_array.is_null() || arg_array.is_undefined() {
            let mut args = Arguments::new(rt, args.at(0), 0);
            return func.call(rt, &mut args);
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

        let arg_array = arg_array.get_jsobject();
        let len = super::get_length(rt, arg_array)?;
        let mut args_ = Arguments::new(rt, args.at(0), len as _);
        for i in 0..len {
            *args_.at_mut(i as _) = arg_array.get(rt, Symbol::Index(i))?;
        }
        return func.call(rt, &mut args_);
    }

    let msg = JsString::new(rt, "Function.prototype.apply is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        rt, msg, None,
    )))
}

pub fn function_call(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this;
    if this.is_callable() {
        let mut obj = this.get_jsobject();
        let func = obj.as_function_mut();

        let args_size = args.size();
        let mut args_ = Arguments::new(
            rt,
            args.at(0),
            if args_size > 1 { args_size - 1 } else { 0 },
        );
        if args_size > 1 {
            for i in 0..args_size - 1 {
                *args_.at_mut(i) = args.at(i + 1);
            }
        }

        return func.call(rt, &mut args_);
    }

    let msg = JsString::new(rt, "Function.prototype.call is not a generic function");
    Err(JsValue::encode_object_value(JsTypeError::new(
        rt, msg, None,
    )))
}
