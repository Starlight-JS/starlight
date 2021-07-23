use std::intrinsics::unlikely;
use std::mem::ManuallyDrop;

use crate::define_jsclass;
use crate::js_method_table;
use crate::jsrt::weak_ref;
use crate::prelude::*;
use crate::vm::builder::Builtin;
use crate::vm::class::JsClass;
use crate::vm::context::Context;
use crate::vm::object::TypedJsObject;
use crate::JsTryFrom;

pub struct JsWeakRef {
    value: WeakRef<JsObject>,
}

extern "C" fn fsz() -> usize {
    std::mem::size_of::<JsWeakRef>()
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    todo!()
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer) {
    todo!()
}
#[allow(improper_ctypes_definitions)]
extern "C" fn trace(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<JsWeakRef>().value.trace(tracer);
}

impl JsClass for JsWeakRef {
    fn class() -> &'static Class {
        define_jsclass!(
            JsWeakRef,
            WeakRef,
            None,
            Some(trace),
            Some(deser),
            Some(ser),
            Some(fsz)
        )
    }
}

pub fn weak_ref_constructor(
    mut ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let target = args.at(0);
    if unlikely(!target.is_jsobject()) {
        return Err(JsValue::new(
            ctx.new_type_error("WeakRef: Target must be an object"),
        ));
    }
    let target = target.get_jsobject();
    let map = ctx.global_data().weak_ref_structure.unwrap();
    let mut weak_ref = JsObject::new(ctx, &map, JsWeakRef::class(), ObjectTag::Ordinary);
    *weak_ref.data::<JsWeakRef>() = ManuallyDrop::new(JsWeakRef {
        value: ctx.heap().make_weak(target),
    });
    Ok(JsValue::new(weak_ref))
}

pub fn weak_ref_prototype_deref(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let weak_ref = TypedJsObject::<JsWeakRef>::try_from(ctx, args.this)?;
    match weak_ref.value.upgrade() {
        Some(value) => Ok(JsValue::new(value)),
        None => Ok(JsValue::encode_undefined_value()),
    }
}

impl Builtin for JsWeakRef {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data().object_prototype.unwrap();
        ctx.global_data.weak_ref_structure = Some(Structure::new_indexed(ctx, None, false));
        let proto_map = ctx
            .global_data
            .weak_ref_structure
            .unwrap()
            .change_prototype_transition(ctx, Some(obj_proto));
        let mut prototype = JsObject::new(ctx, &proto_map, JsObject::class(), ObjectTag::Ordinary);
        ctx.global_data
            .weak_ref_structure
            .unwrap()
            .change_prototype_with_no_transition(prototype);

        let mut constructor =
            JsNativeFunction::new(ctx, S_WEAK_REF.intern(), weak_ref::weak_ref_constructor, 1);

        def_native_property!(ctx, prototype, constructor, constructor)?;
        def_native_property!(ctx, constructor, prototype, prototype)?;

        def_native_method!(ctx, prototype, deref, weak_ref::weak_ref_prototype_deref, 0)?;

        ctx.global_data.weak_ref_prototype = Some(prototype);

        let mut global_object = ctx.global_object();

        def_native_property!(ctx, global_object, WeakRef, constructor)?;
        Ok(())
    }
}
