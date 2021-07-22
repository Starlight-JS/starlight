use std::intrinsics::unlikely;

use crate::{
    constant::S_OBJECT,
    gc::cell::GcPointer,
    vm::{
        arguments::Arguments,
        array::*,
        attributes::*,
        builder::Builtin,
        class::JsClass,
        context::Context,
        error::JsTypeError,
        function::JsNativeFunction,
        object::{JsObject, ObjectTag, *},
        property_descriptor::DataDescriptor,
        string::JsString,
        structure::Structure,
        symbol_table::*,
        value::{JsValue, Undefined},
    },
};

pub fn object_get_prototype_of(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.at(0);
    if unlikely(this.is_undefined() || this.is_null()) {
        return Err(JsValue::new(
            ctx.new_type_error("Object.getPrototypeOf requires object argument"),
        ));
    }

    let object = this.to_object(ctx)?;
    Ok(match object.prototype() {
        Some(proto) => JsValue::new(*proto),
        None => JsValue::encode_null_value(),
    })
}

pub fn object_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let this_binding = args.this;

    if this_binding.is_undefined() {
        return Ok(JsValue::encode_object_value(JsString::new(
            ctx,
            "[object Undefined]",
        )));
    } else if this_binding.is_null() {
        return Ok(JsValue::encode_object_value(JsString::new(
            ctx,
            "[object Null]",
        )));
    }
    let obj = this_binding.to_object(ctx)?;

    let s = format!("[object {}]", obj.class.name);
    Ok(JsValue::encode_object_value(JsString::new(ctx, s)))
}

pub fn object_create(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let stack = ctx.shadowstack();
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
                Structure::new_unique_indexed(ctx, *prototype, false)
            );
            let res = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);
            if !args.at(1).is_undefined() {
                let mut res_val = JsValue::new(res);
                let mut args_ = [res_val, properties];
                let mut ctor = ctx.global_data().object_constructor.unwrap();
                let props = ctor.get(ctx, "___defineProperties___".intern())?;

                assert!(props.is_callable());

                return props.get_jsobject().as_function_mut().call(
                    ctx,
                    &mut Arguments::new(JsValue::encode_undefined_value(), &mut args_),
                    JsValue::encode_undefined_value(),
                );
            }

            return Ok(JsValue::encode_object_value(res));
        }
    }

    let msg = JsString::new(ctx, "Object.create requires Object or null argument");
    return Err(JsValue::encode_object_value(JsTypeError::new(
        ctx, msg, None,
    )));
}

pub fn object_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let val = args.at(0);
    if args.ctor_call {
        if val.is_jsstring() || val.is_number() || val.is_bool() {
            return val.to_object(ctx).map(JsValue::encode_object_value);
        }
        return Ok(JsValue::encode_object_value(JsObject::new_empty(ctx)));
    } else if val.is_undefined() || val.is_null() {
        return Ok(JsValue::encode_object_value(JsObject::new_empty(ctx)));
    } else {
        return val.to_object(ctx).map(JsValue::encode_object_value);
    }
}

pub fn object_define_property(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());

            let name = args.at(1).to_symbol(ctx)?;
            let attr = args.at(2);
            let desc = super::to_property_descriptor(ctx, attr)?;

            obj.define_own_property(ctx, name, &desc, true)?;
            return Ok(JsValue::new(*obj));
        }
    }

    return Err(JsValue::new(
        ctx.new_type_error("Object.defineProperty requires Object argument"),
    ));
}

pub fn object_has_own_property(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::new(false));
    }
    let prop = args.at(0).to_symbol(ctx)?;
    let mut obj = args.this.to_object(ctx)?;
    Ok(JsValue::new(obj.get_own_property(ctx, prop).is_some()))
}

pub fn object_get_own_property_descriptor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    if args.size() < 2 {
        return Ok(JsValue::new(Undefined));
    }
    let first = args.at(0);
    let prop = args.at(1);
    if first.is_jsobject() {
        letroot!(obj = stack, first.get_jsobject());
        let name = prop.to_symbol(ctx)?;

        match obj.get_own_property(ctx, name) {
            Some(property_descriptor) => {
                letroot!(res = stack, JsObject::new_empty(ctx));
                res.define_own_property(
                    ctx,
                    "configurable".intern(),
                    &*DataDescriptor::new(
                        JsValue::new(property_descriptor.is_configurable()),
                        W | C,
                    ),
                    false,
                )?;
                res.define_own_property(
                    ctx,
                    "enumerable".intern(),
                    &*DataDescriptor::new(JsValue::new(property_descriptor.is_enumerable()), W | C),
                    false,
                )?;
                if property_descriptor.is_data() {
                    res.define_own_property(
                        ctx,
                        "value".intern(),
                        &*DataDescriptor::new(JsValue::new(property_descriptor.value()), W | C),
                        false,
                    )?;
                    res.define_own_property(
                        ctx,
                        "writable".intern(),
                        &*DataDescriptor::new(
                            JsValue::new(property_descriptor.is_writable()),
                            W | C,
                        ),
                        false,
                    )?;
                } else {
                    let getter = property_descriptor.getter();
                    let setter = property_descriptor.setter();

                    res.define_own_property(
                        ctx,
                        "get".intern(),
                        &*DataDescriptor::new(getter, W | C),
                        false,
                    )?;
                    res.define_own_property(
                        ctx,
                        "set".intern(),
                        &*DataDescriptor::new(setter, W | C),
                        false,
                    )?;
                }
                Ok(JsValue::encode_object_value(*res))
            }
            None => Ok(JsValue::new(Undefined)),
        }
    } else {
        Err(JsValue::new(ctx.new_type_error(
            "Object.getOwnPropertyDescriptor requires object argument",
        )))
    }
}

pub fn object_property_is_enumerable(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() < 1 {
        return Ok(JsValue::encode_bool_value(false));
    }
    let prop = args.at(0).to_symbol(ctx)?;
    let mut obj = args.this.to_object(ctx)?;
    let desc = obj.get_own_property(ctx, prop);
    if desc.is_none() {
        return Ok(JsValue::encode_bool_value(false));
    } else {
        let desc = desc.unwrap();
        return Ok(JsValue::encode_bool_value(desc.is_enumerable()));
    }
}

pub fn object_keys(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    if args.size() != 0 {
        let first = args.at(0);
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                ctx,
                &mut |name, _| names.push(name),
                EnumerationMode::Default,
            );
            letroot!(arr = stack, JsArray::new(ctx, names.len() as _));

            for (i, name) in names.iter().enumerate() {
                let desc = ctx.description(*name);
                let name = JsString::new(ctx, desc);
                arr.put(ctx, Symbol::Index(i as _), JsValue::new(name), false)?;
            }
            return Ok(JsValue::new(*arr));
        }
    }

    Err(JsValue::new(
        ctx.new_type_error("Object.keys requires object argument"),
    ))
}

pub fn object_freeze(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.freeze(ctx)?;
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(
        ctx.new_type_error("Object.freeze requires object argument"),
    ))
}

pub fn object_seal(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.seal(ctx)?;
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(
        ctx.new_type_error("Object.seal requires object argument"),
    ))
}
pub fn object_prevent_extensions(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            obj.change_extensible(ctx, false);
            return Ok(JsValue::new(*obj));
        }
    }
    Err(JsValue::new(ctx.new_type_error(
        "Object.preventExtensions requires object argument",
    )))
}

pub fn object_is_sealed(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                ctx,
                &mut |name, _| names.push(name),
                EnumerationMode::IncludeNotEnumerable,
            );
            for name in names {
                let desc = obj.get_own_property(ctx, name).unwrap();
                if desc.is_configurable() {
                    return Ok(JsValue::new(false));
                }
            }
            return Ok(JsValue::new(!obj.is_extensible()));
        }
    }
    Err(JsValue::new(
        ctx.new_type_error("Object.isSealed requires object argument"),
    ))
}

pub fn object_is_frozen(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());
            let mut names = vec![];
            obj.get_own_property_names(
                ctx,
                &mut |name, _| names.push(name),
                EnumerationMode::IncludeNotEnumerable,
            );
            for name in names {
                let desc = obj.get_own_property(ctx, name).unwrap();
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
        ctx.new_type_error("Object.isFrozen requires object argument"),
    ))
}

pub fn object_is_extensible(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let first = args.at(0);
        let stack = ctx.shadowstack();
        if first.is_jsobject() {
            letroot!(obj = stack, first.get_jsobject());

            return Ok(JsValue::new(obj.is_extensible()));
        }
    }
    Err(JsValue::new(ctx.new_type_error(
        "Object.isExtensible requires object argument",
    )))
}

impl Builtin for JsObject {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let structure = Structure::new_unique_indexed(ctx, None, false);
        let mut prototype = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);
        ctx.global_data.object_prototype = Some(prototype);

        ctx.global_data.empty_object_struct = Some(Structure::new_indexed(ctx, None, false));
        ctx.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(prototype);
        ctx.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(prototype);

        ctx.global_data.function_struct = Some(Structure::new_indexed(ctx, None, false));

        let name = S_OBJECT.intern();
        let mut constructor = JsNativeFunction::new(ctx, name, object_constructor, 1);
        ctx.global_data.object_constructor = Some(constructor);

        def_native_method!(
            ctx,
            constructor,
            defineProperty,
            object_define_property,
            3,
            NONE
        )?;

        def_native_method!(ctx, constructor, seal, object_seal, 1, NONE)?;

        def_native_method!(ctx, constructor, freeze, object_freeze, 1, NONE)?;

        def_native_method!(ctx, constructor, isSealed, object_is_sealed, 1, NONE)?;

        def_native_method!(ctx, constructor, isFrozen, object_is_frozen, 1, NONE)?;

        def_native_method!(
            ctx,
            constructor,
            isExtensible,
            object_is_extensible,
            1,
            NONE
        )?;

        def_native_method!(
            ctx,
            constructor,
            getPrototypeOf,
            object_get_prototype_of,
            1,
            NONE
        )?;

        def_native_method!(
            ctx,
            constructor,
            preventExtensions,
            object_prevent_extensions,
            1,
            NONE
        )?;

        def_native_method!(ctx, constructor, keys, object_keys, 1, NONE)?;

        def_native_method!(
            ctx,
            constructor,
            getOwnPropertyDescriptor,
            object_get_own_property_descriptor,
            2,
            NONE
        )?;

        def_native_method!(ctx, constructor, create, object_create, 3, NONE)?;

        def_native_property!(ctx, constructor, prototype, prototype, NONE)?;

        def_native_property!(ctx, prototype, constructor, constructor, W | C)?;

        def_native_method!(ctx, prototype, toString, object_to_string, 0, W | C)?;

        def_native_method!(
            ctx,
            prototype,
            hasOwnProperty,
            object_has_own_property,
            1,
            W | C
        )?;

        def_native_method!(
            ctx,
            prototype,
            propertyIsEnumerable,
            object_property_is_enumerable,
            1,
            W | C
        )?;

        let mut global_object = ctx.global_object();
        def_native_property!(ctx, global_object, Object, constructor, W | C)?;

        constructor
            .get(ctx, "prototype")
            .unwrap()
            .to_object(ctx)
            .unwrap()
            .get(ctx, "hasOwnProperty")
            .unwrap()
            .to_object(ctx)
            .unwrap()
            .as_function_mut();

        def_native_property!(ctx, global_object, globalThis, global_object)?;

        Ok(())
    }
}
