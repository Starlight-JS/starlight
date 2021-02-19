use crate::{
    gc::handle::Handle,
    runtime::{
        arguments::Arguments,
        error::JsTypeError,
        object::{JsObject, ObjectTag},
        string::JsString,
        structure::Structure,
        value::JsValue,
    },
    vm::VirtualMachine,
};

pub fn object_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let this_binding = args.this;

    if this_binding.is_undefined() {
        return Ok(JsValue::new(JsString::new(vm, "[object Undefined]")));
    } else if this_binding.is_null() {
        return Ok(JsValue::new(JsString::new(vm, "[object Undefined]")));
    }
    let obj = this_binding.to_object(vm)?.root(vm.space());

    let s = format!("[object {}]", obj.class().name);
    Ok(JsValue::new(JsString::new(vm, s)))
}

pub fn object_create(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_object() || first.is_null() {
            let prototype = if first.is_object() {
                Some(first.as_object().root(vm.space()))
            } else {
                None
            };
            let structure =
                Structure::new_unique_indexed(vm, prototype.map(|x| *x), false).root(vm.space());
            let res = JsObject::new(vm, *structure, JsObject::get_class(), ObjectTag::Ordinary)
                .root(vm.space());
            if !args.at(1).is_undefined() {
                todo!("define properties");
            }

            return Ok(JsValue::new(*res));
        }
    }

    let msg = JsString::new(vm, "Object.create requires Object or null argument").root(vm.space());
    return Err(JsValue::new(JsTypeError::new(vm, *msg, None)));
}

pub fn object_constructor(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.ctor_call {
        let val = Handle::new(vm.space(), args.at(0));
        if val.is_string() || val.is_number() || val.is_boolean() {
            return val.to_object(vm).map(|x| JsValue::new(x));
        }
        return Ok(JsValue::new(JsObject::new_empty(vm)));
    } else {
        let val = args.at(0);
        if val.is_undefined_or_null() {
            return Ok(JsValue::new(JsObject::new_empty(vm)));
        } else {
            return Handle::new(vm.space(), val)
                .to_object(vm)
                .map(|x| JsValue::new(x));
        }
    }
}
