use crate::{
    vm::Runtime,
    vm::{
        arguments::Arguments, error::JsTypeError, error::*, object::JsObject, slot::*,
        string::JsString, symbol_table::*, value::JsValue,
    },
};

pub fn error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsError::new(vm, msg, None)))
}

pub fn eval_error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        vm, msg, None,
    )))
}

pub fn reference_error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsReferenceError::new(
        vm, msg, None,
    )))
}

pub fn type_error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsTypeError::new(
        vm, msg, None,
    )))
}

pub fn syntax_error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsEvalError::new(
        vm, msg, None,
    )))
}

pub fn range_error_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let message = args.at(0).to_string(vm)?;
    let msg = JsString::new(vm, message);
    Ok(JsValue::encode_object_value(JsRangeError::new(
        vm, msg, None,
    )))
}

/// section 15.11.4.4 Error.prototype.toString()
pub fn error_to_string(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    let stack = vm.shadowstack();
    if obj.is_jsobject() {
        letroot!(obj = stack, unsafe {
            obj.get_object().downcast_unchecked::<JsObject>()
        });
        let name;
        {
            let mut slot = Slot::new();
            let target = obj.get_slot(vm, "name".intern(), &mut slot)?;
            if target.is_undefined() {
                name = "UnknownError".to_owned();
            } else {
                name = target.to_string(vm)?;
            }
        }
        let msg;
        {
            let target = obj.get(vm, "message".intern())?;
            if target.is_undefined() {
                msg = String::new();
            } else {
                msg = target.to_string(vm)?;
            }
        }

        if name.is_empty() {
            return Ok(JsValue::encode_object_value(JsString::new(vm, msg)));
        }
        if msg.is_empty() {
            return Ok(JsValue::encode_object_value(JsString::new(vm, name)));
        }

        Ok(JsValue::encode_object_value(JsString::new(
            vm,
            format!("{}: {}", name, msg),
        )))
    } else {
        let msg = JsString::new(vm, "Base must be an object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            vm, msg, None,
        )));
    }
}
