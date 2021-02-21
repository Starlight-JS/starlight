use crate::{
    runtime::{
        arguments::Arguments,
        error::JsTypeError,
        function::{FuncType, JsBoundFunction, JsFunction},
        gc_array::GcArray,
        slot::Slot,
        string::JsString,
        symbol::Symbol,
        value::JsValue,
    },
    vm::VirtualMachine,
};

pub fn function_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    if obj.is_callable() {
        let func = obj.to_object(vm)?;
        let mut slot = Slot::new();
        let mut fmt = "function ".to_string();
        if func.get_own_property_slot(vm, Symbol::name(), &mut slot) {
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
        return Ok(JsValue::new(JsString::new(vm, fmt)));
    }
    let msg = JsString::new(vm, "Function.prototype.toString is not generic");
    return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
}

pub fn function_prototype(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let _ = vm;
    let _ = args;
    Ok(JsValue::undefined())
}

pub fn function_bind(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this;

    if obj.is_callable() {
        let mut vals = GcArray::new(
            vm,
            if args.size() == 0 { 0 } else { args.size() - 1 },
            JsValue::undefined(),
        );
        for i in 1..args.size() {
            vals[i - 1] = args.at(i);
        }
        let f = JsFunction::new(
            vm,
            FuncType::Bound(JsBoundFunction {
                args: vals,
                this: args.at(0),
                target: obj.as_object(),
            }),
            false,
        );

        return Ok(JsValue::new(f));
    }
    let msg = JsString::new(vm, "Function.prototype.bind is not generic");
    return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
}
