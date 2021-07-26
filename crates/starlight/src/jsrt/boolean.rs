use std::mem::ManuallyDrop;

use crate::{
    define_jsclass,
    prelude::*,
    vm::{
        builder::Builtin, class::Class, context::Context, method_table::*, object::TypedJsObject,
    },
    JsTryFrom,
};
pub struct JsBoolean {
    data: bool,
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer) {
    todo!()
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    todo!()
}
extern "C" fn fsz() -> usize {
    std::mem::size_of::<JsBoolean>()
}

impl JsClass for JsBoolean {
    fn class() -> &'static Class {
        define_jsclass!(
            JsBoolean,
            Boolean,
            None,
            None,
            Some(deser),
            Some(ser),
            Some(fsz)
        )
    }
}

impl JsBoolean {
    pub fn new(ctx: GcPointer<Context>, val: bool) -> GcPointer<JsObject> {
        let proto = ctx.global_data().boolean_structure.unwrap();
        let mut obj = JsObject::new(ctx, &proto, Self::class(), ObjectTag::Ordinary);
        *obj.data::<Self>() = ManuallyDrop::new(Self { data: val });
        obj
    }
}

fn this_boolean_value(val: JsValue, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
    if val.is_bool() {
        return Ok(val.get_bool());
    }
    let obj = TypedJsObject::<JsBoolean>::try_from(ctx, val)?;
    Ok(obj.data)
}

pub fn boolean_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let data = args.at(0).to_boolean();
    if !args.ctor_call {
        return Ok(JsValue::new(data));
    }
    Ok(JsValue::new(JsBoolean::new(ctx, data)))
}

pub fn boolean_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(JsString::new(
        ctx,
        format!("{}", this_boolean_value(args.this, ctx)?),
    )))
}

pub fn boolean_value_of(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(this_boolean_value(args.this, ctx)?))
}

impl Builtin for JsBoolean {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let mut map = Structure::new_indexed(ctx, None, false);
        ctx.global_data.boolean_structure = Some(map);
        let obj_proto = ctx.global_data().get_object_prototype();
        let structure = Structure::new_unique_indexed(ctx, Some(obj_proto), false);
        let mut proto = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);
        map.change_prototype_with_no_transition(proto);

        let mut ctor = JsNativeFunction::new(ctx, "Boolean".intern(), boolean_constructor, 1);

        def_native_property!(ctx, ctor, prototype, proto, NONE)?;

        def_native_method!(ctx, proto, toString, boolean_to_string, 0)?;

        def_native_method!(ctx, proto, valueOf, boolean_value_of, 0)?;

        def_native_property!(ctx, proto, constructor, ctor, W | C)?;

        ctx.global_data.boolean_prototype = Some(proto);

        let mut global_object = ctx.global_object();
        def_native_property!(ctx, global_object, Boolean, ctor)?;
        Ok(())
    }
}
