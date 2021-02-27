use crate::{
    vm::Runtime,
    vm::{
        arguments::Arguments, array_storage::ArrayStorage, error::JsTypeError, function::*,
        slot::*, string::JsString, symbol_table::Internable, value::JsValue,
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
