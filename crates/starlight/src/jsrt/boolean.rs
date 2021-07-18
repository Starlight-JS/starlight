use std::mem::ManuallyDrop;

use crate::{
    define_jsclass_with_symbol,
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
impl BooleanObject {
    define_jsclass_with_symbol!(
        JsObject,
        Boolean,
        Object,
        None,
        None,
        Some(deser),
        Some(ser),
        Some(fsz)
    );

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
    pub(crate) fn init_boolean_in_global_object(mut self) {
        let ctor = self
            .global_data
            .boolean_prototype
            .unwrap()
            .get(self, "constructor".intern())
            .unwrap_or_else(|_| unreachable!());
        self.global_object()
            .put(self, "Boolean".intern(), JsValue::new(ctor), false)
            .unwrap_or_else(|_| unreachable!());
    }
    pub(crate) fn init_boolean_in_global_data(mut self) {
        let mut map = Structure::new_indexed(self, None, false);
        self.global_data.boolean_structure = Some(map);
        let obj_proto = self.global_data().get_object_prototype();
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        map.change_prototype_with_no_transition(proto);

        let mut ctor = JsNativeFunction::new(self, "Boolean".intern(), boolean_constructor, 1);
        ctor.define_own_property(
            self,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::new(proto), NONE),
            false,
        )
        .unwrap_or_else(|_| unreachable!());

        let to_string = JsNativeFunction::new(self, "toString".intern(), boolean_to_string, 0);
        proto
            .put(self, "toString".intern(), JsValue::new(to_string), false)
            .unwrap_or_else(|_| unreachable!());
        let value_of = JsNativeFunction::new(self, "valueOf".intern(), boolean_value_of, 0);
        proto
            .put(self, "valueOf".intern(), JsValue::new(value_of), false)
            .unwrap_or_else(|_| unreachable!());
        proto
            .define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::new(ctor), W | C),
                false,
            )
            .unwrap_or_else(|_| unreachable!());

        self.global_data.boolean_prototype = Some(proto);
    }
}
