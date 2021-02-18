use crate::{
    runtime::{
        arguments::Arguments,
        array::JsArray,
        object::{JsObject, ObjectTag},
        value::JsValue,
    },
    vm::VirtualMachine,
};

pub fn array_ctor(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let size = args.size();
    todo!()
}

pub fn array_is_array(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::new(false));
    }
    let val = args.at(0);
    if !val.is_object() {
        return Ok(JsValue::new(false));
    }
    Ok(JsValue::new(val.as_object().tag() == ObjectTag::Array))
}

pub fn array_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    todo!()
}
