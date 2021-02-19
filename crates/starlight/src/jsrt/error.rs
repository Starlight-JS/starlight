use crate::{
    runtime::{
        arguments::Arguments,
        error::{JsError, JsEvalError, JsRangeError, JsReferenceError, JsTypeError},
        object::JsObject,
        slot::Slot,
        string::JsString,
        symbol::Symbol,
        value::JsValue,
    },
    vm::VirtualMachine,
};

pub fn error_constructor(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsError::new(vm, *msg, None)))
}

pub fn eval_error_constructor(
    vm: &mut VirtualMachine,

    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsEvalError::new(vm, *msg, None)))
}

pub fn reference_error_constructor(
    vm: &mut VirtualMachine,

    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsReferenceError::new(vm, *msg, None)))
}

pub fn type_error_constructor(
    vm: &mut VirtualMachine,

    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsTypeError::new(vm, *msg, None)))
}

pub fn syntax_error_constructor(
    vm: &mut VirtualMachine,

    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsEvalError::new(vm, *msg, None)))
}

pub fn range_error_constructor(
    vm: &mut VirtualMachine,

    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message).root(vm.space());
    Ok(JsValue::new(JsRangeError::new(vm, *msg, None)))
}

/// section 15.11.4.4 Error.prototype.toString()
pub fn error_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;

    if obj.is_cell() && obj.as_cell().is::<JsObject>() {
        let obj = unsafe { obj.as_cell().downcast_unchecked::<JsObject>() };
        let name;
        {
            let mut slot = Slot::new();
            let target = obj.get_slot(vm, Symbol::name(), &mut slot)?;
            if target.is_undefined() {
                name = "UnknownError".to_owned();
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
