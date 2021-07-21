use std::mem::ManuallyDrop;

use crate::{
    constant::S_CONSTURCTOR,
    define_jsclass,
    prelude::*,
    vm::{class::JsClass, context::Context, method_table::*, object::TypedJsObject},
    JsTryFrom,
};
pub struct BooleanObject {
    data: bool,
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer) {
    todo!()
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    todo!()
}
extern "C" fn fsz() -> usize {
    std::mem::size_of::<BooleanObject>()
}
define_jsclass!(
    BooleanObject,
    Boolean,
    Object,
    None,
    None,
    Some(deser),
    Some(ser),
    Some(fsz)
);

impl BooleanObject {
    pub fn new(ctx: GcPointer<Context>, val: bool) -> GcPointer<JsObject> {
        let proto = ctx.global_data().boolean_structure.unwrap();
        let mut obj = JsObject::new(ctx, &proto, Self::get_class(), ObjectTag::Ordinary);
        *obj.data::<Self>() = ManuallyDrop::new(Self { data: val });
        obj
    }
}

fn this_boolean_value(val: JsValue, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
    if val.is_bool() {
        return Ok(val.get_bool());
    }
    let obj = TypedJsObject::<BooleanObject>::try_from(ctx, val)?;
    Ok(obj.data)
}

impl JsClass for BooleanObject {
    fn class() -> &'static Class {
        Self::get_class()
    }
}

pub fn boolean_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let data = args.at(0).to_boolean();
    if !args.ctor_call {
        return Ok(JsValue::new(data));
    }
    Ok(JsValue::new(BooleanObject::new(ctx, data)))
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

impl GcPointer<Context> {
    pub(crate) fn init_boolean_in_global_object(mut self) -> Result<(), JsValue> {
        let ctor = self
            .global_data
            .boolean_prototype
            .unwrap()
            .get(self, S_CONSTURCTOR.intern())
            .unwrap_or_else(|_| unreachable!());

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, Boolean, ctor)?;
        Ok(())
    }
    pub(crate) fn init_boolean_in_global_data(mut self) -> Result<(), JsValue> {
        let mut map = Structure::new_indexed(self, None, false);
        self.global_data.boolean_structure = Some(map);
        let obj_proto = self.global_data().get_object_prototype();
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        map.change_prototype_with_no_transition(proto);

        let mut ctor = JsNativeFunction::new(self, "Boolean".intern(), boolean_constructor, 1);

        def_native_property!(self, ctor, prototype, proto, NONE)?;

        def_native_method!(self, proto, toString, boolean_to_string, 0)?;

        def_native_method!(self, proto, valueOf, boolean_value_of, 0)?;

        def_native_property!(self, proto, constructor, ctor, W | C)?;

        self.global_data.boolean_prototype = Some(proto);
        Ok(())
    }
}
