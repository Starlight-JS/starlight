use crate::{
    runtime::{
        arguments::Arguments, object::JsObject, string::JsString, symbol::Symbol, value::JsValue,
    },
    vm::VirtualMachine,
};

pub fn error_constructor(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    todo!()
}

pub fn eval_error_constructor(
    vm: &mut VirtualMachine,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!()
}

pub fn reference_error_constructor(
    vm: &mut VirtualMachine,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!()
}

pub fn type_error_constructor(
    vm: &mut VirtualMachine,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!()
}

pub fn syntax_error_constructor(
    vm: &mut VirtualMachine,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!()
}

pub fn range_error_constructor(
    vm: &mut VirtualMachine,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!()
}

/// section 15.11.4.4 Error.prototype.toString()
pub fn error_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;

    if obj.is_cell() && obj.as_cell().is::<JsObject>() {
        let obj = unsafe { obj.as_cell().downcast_unchecked::<JsObject>() };
        let name;
        {
            let target = obj.get(vm, Symbol::name())?;
            if target.is_undefined() {
                name = "Error".to_owned();
            } else {
                name = target.to_string(vm)?;
            }
        }
        let msg;
        {
            let target = obj.get(vm, Symbol::message())?;
            if target.is_undefined() {
                msg = String::new();
            } else {
                msg = target.to_string(vm)?;
            }
        }

        if name.is_empty() {
            return Ok(JsValue::new(JsString::new(vm, msg)));
        }
        if msg.is_empty() {
            return Ok(JsValue::new(JsString::new(vm, name)));
        }

        Ok(JsValue::new(JsString::new(
            vm,
            format!("{}: {}", name, msg),
        )))
    } else {
        todo!()
    }
}
