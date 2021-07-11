use std::intrinsics::unlikely;

use crate::{
    vm::Runtime,
    vm::{
        arguments::Arguments,
        array::*,
        attributes::*,
        error::JsTypeError,
        object::{JsObject, ObjectTag, *},
        property_descriptor::DataDescriptor,
        string::JsString,
        structure::Structure,
        symbol_table::*,
        value::{JsValue, Undefined},
    },
};

pub fn object_get_prototype_of(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.at(0);
    if unlikely(this.is_undefined() || this.is_null()) {
        return Err(JsValue::new(
            vm.new_type_error("Object.getPrototypeOf requires object argument"),
        ));
    }

    let object = this.to_object(vm)?;
    Ok(match object.prototype(vm) {
        Some(proto) => JsValue::new(*proto),
        None => JsValue::encode_null_value(),
    })
}

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
            "[object Null]",
        )));
    }
    let obj = this_binding.to_object(vm)?;

    let s = format!("[object {}]", obj.class().name);
    Ok(JsValue::encode_object_value(JsString::new(vm, s)))
}

pub fn object_create(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let stack = vm.shadowstack();
        let first = args.at(0);
        let properties = args.at(1);
        if first.is_object() || first.is_null() {
            letroot!(
                prototype = stack,
                if first.is_jsobject() {
                    Some(unsafe { first.get_object().downcast_unchecked::<JsObject>() })
                } else {
                    None
                }
            );
            letroot!(
                structure = stack,
                Structure::new_unique_indexed(vm, *prototype, false)
            );
            let res = JsObject::new(vm, &structure, JsObject::get_class(), ObjectTag::Ordinary);
            if !args.at(1).is_undefined() {
                let mut res_val = JsValue::new(res);
                let mut args_ = [res_val, properties];
                let props = vm
                    .global_data()
                    .object_prototype
                    .unwrap()
                    .get(vm, "defineProperties".intern())?;
                assert!(props.is_callable());

                return props.get_jsobject().as_function_mut().call(
                    vm,
                    &mut Arguments::new(JsValue::encode_undefined_value(), &mut args_),
                    JsValue::encode_undefined_value(),
                );
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
    let val = args.at(0);
    if args.ctor_call {
        if val.is_jsstring() || val.is_number() || val.is_bool() {
            return val.to_object(vm).map(JsValue::encode_object_value);
        }
        return Ok(JsValue::encode_object_value(JsObject::new_empty(vm)));
    } else if val.is_undefined() || val.is_null() {
        return Ok(JsValue::encode_object_value(JsObject::new_empty(vm)));
    } else {
        return val.to_object(vm).map(JsValue::encode_object_value);
    }
}

pub fn object_define_property(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());

            let name = args.at(1).to_symbol(vm)?;
            let attr = args.at(2);
            let desc = super::to_property_descriptor(vm, attr)?;

            obj.define_own_property(vm, name, &desc, true)?;
            return Ok(JsValue::new(*obj));
        }
    }

    return Err(JsValue::new(
        vm.new_type_error("Object.defineProperty requires Object argument"),
    ));
}

pub fn has_own_property(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::new(false));
    }
    let prop = args.at(0).to_symbol(vm)?;
    let mut obj = args.this.to_object(vm)?;
    Ok(JsValue::new(obj.get_own_property(vm, prop).is_some()))
}

pub fn object_get_own_property_descriptor(
    vm: &mut Runtime,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    if args.size() < 2 {
        return Ok(JsValue::new(Undefined));
    }
    let first = args.at(0);
    let prop = args.at(1);
    if first.is_jsobject() {
        letroot!(obj = stack, first.get_jsobject());
        let name = prop.to_symbol(vm)?;

        match obj.get_own_property(vm, name) {
            Some(property_descriptor) => {
                letroot!(res = stack, JsObject::new_empty(vm));
                res.define_own_property(
                    vm,
                    "configurable".intern(),
                    &*DataDescriptor::new(
                        JsValue::new(property_descriptor.is_configurable()),
                        W | C,
                    ),
                    false,
                )?;
                res.define_own_property(
                    vm,
                    "enumerable".intern(),
                    &*DataDescriptor::new(JsValue::new(property_descriptor.is_enumerable()), W | C),
                    false,
                )?;
                if property_descriptor.is_data() {
                    res.define_own_property(
                        vm,
                        "value".intern(),
                        &*DataDescriptor::new(JsValue::new(property_descriptor.value()), W | C),
                        false,
                    )?;
                    res.define_own_property(
                        vm,
                        "writable".intern(),
                        &*DataDescriptor::new(
                            JsValue::new(property_descriptor.is_writable()),
                            W | C,
                        ),
                        false,
                    )?;
                } else {
                    letroot!(getter = stack, property_descriptor.getter());
                    letroot!(setter = stack, property_descriptor.setter());

                    res.define_own_property(
                        vm,
                        "get".intern(),
                        &*DataDescriptor::new(JsValue::new(*getter), W | C),
                        false,
                    )?;
                    res.define_own_property(
                        vm,
                        "set".intern(),
                        &*DataDescriptor::new(JsValue::new(*setter), W | C),
                        false,
                    )?;
                }
                Ok(JsValue::encode_object_value(*res))
            }
            None => Ok(JsValue::new(Undefined)),
        }
    } else {
        Err(JsValue::new(vm.new_type_error(
            "Object.getOwnPropertyDescriptor requires object argument",
        )))
    }
}

pub fn object_keys(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                vm,
                &mut |name, _| names.push(name),
                EnumerationMode::Default,
            );
            letroot!(arr = stack, JsArray::new(vm, names.len() as _));

            for (i, name) in names.iter().enumerate() {
                let desc = vm.description(*name);
                let name = JsString::new(vm, desc);
                arr.put(vm, Symbol::Index(i as _), JsValue::new(name), false)?;
            }
            return Ok(JsValue::new(*arr));
        }
    }

    Err(JsValue::new(
        vm.new_type_error("Object.keys requires object argument"),
    ))
}

pub fn object_freeze(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.freeze(vm)?;
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(
        vm.new_type_error("Object.freeze requires object argument"),
    ))
}

pub fn object_seal(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.seal(vm)?;
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(
        vm.new_type_error("Object.seal requires object argument"),
    ))
}
pub fn object_prevent_extensions(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.change_extensible(vm, false);
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(vm.new_type_error(
        "Object.preventExtensions requires object argument",
    )))
}

pub fn object_is_sealed(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                vm,
                &mut |name, _| names.push(name),
                EnumerationMode::IncludeNotEnumerable,
            );
            for name in names {
                let desc = obj.get_own_property(vm, name).unwrap();
                if desc.is_configurable() {
                    return Ok(JsValue::new(false));
                }
            }
            return Ok(JsValue::new(!obj.is_extensible()));
        }
    }
    Err(JsValue::new(
        vm.new_type_error("Object.isSealed requires object argument"),
    ))
}

pub fn object_is_frozen(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                vm,
                &mut |name, _| names.push(name),
                EnumerationMode::IncludeNotEnumerable,
            );
            for name in names {
                let desc = obj.get_own_property(vm, name).unwrap();
                if desc.is_configurable() {
                    return Ok(JsValue::new(false));
                }
                if desc.is_data() && desc.is_writable() {
                    return Ok(JsValue::new(false));
                }
            }
            return Ok(JsValue::new(!obj.is_extensible()));
        }
    }
    Err(JsValue::new(
        vm.new_type_error("Object.isFrozen requires object argument"),
    ))
}

pub fn object_is_extensible(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = vm.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());

            return Ok(JsValue::new(obj.is_extensible()));
        }
    }
    Err(JsValue::new(vm.new_type_error(
        "Object.isExtensible requires object argument",
    )))
}
