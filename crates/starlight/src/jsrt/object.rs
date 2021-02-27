use crate::{
    vm::Runtime,
    vm::{
        arguments::Arguments,
        error::JsTypeError,
        object::{JsObject, ObjectTag},
        string::JsString,
        structure::Structure,
        value::JsValue,
    },
};

pub fn object_to_string(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this_binding = args.this;

    if this_binding.is_undefined() {
        return Ok(JsValue::encode_object_value(JsString::new(
            vm,
            "[object Undefined]",
        )));
    } else if this_binding.is_null() {
        return Ok(JsValue::encode_object_value(JsString::new(
            vm,
            "[object Undefined]",
        )));
    }
    let obj = this_binding.to_object(vm)?;

    let s = format!("[object {}]", obj.class().name);
    Ok(JsValue::encode_object_value(JsString::new(vm, s)))
}

pub fn object_create(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_object() || first.is_null() {
            let prototype = if first.is_jsobject() {
                Some(unsafe { first.get_object().downcast_unchecked::<JsObject>() })
            } else {
                None
            };
            let structure = Structure::new_unique_indexed(vm, prototype, false);
            let res = JsObject::new(vm, structure, JsObject::get_class(), ObjectTag::Ordinary);
            if !args.at(1).is_undefined() {
                todo!("define properties");
            }

            return Ok(JsValue::encode_object_value(res));
        }
    }

    let msg = JsString::new(vm, "Object.create requires Object or null argument");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        vm, msg, None,
    )));
}

pub fn object_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.ctor_call {
        let val = args.at(0);
        if val.is_string() || val.is_number() || val.is_bool() {
            return val.to_object(vm).map(|x| JsValue::encode_object_value(x));
        }
        return Ok(JsValue::encode_object_value(JsObject::new_empty(vm)));
    } else {
        let val = args.at(0);
        if val.is_undefined() || val.is_null() {
            return Ok(JsValue::encode_object_value(JsObject::new_empty(vm)));
        } else {
            return val.to_object(vm).map(|x| JsValue::encode_object_value(x));
        }
    }
}
