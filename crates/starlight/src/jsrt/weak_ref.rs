use std::intrinsics::unlikely;
use std::mem::ManuallyDrop;

use crate::define_jsclass;
use crate::js_method_table;
use crate::prelude::*;
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

define_jsclass!(
    JsWeakRef,
    WeakRef,
    Object,
    None,
    Some(trace),
    Some(deser),
    Some(ser),
    Some(fsz)
);

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
    let mut weak_ref = JsObject::new(ctx, &map, JsWeakRef::get_class(), ObjectTag::Ordinary);
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

impl JsClass for JsWeakRef {
    fn class() -> &'static Class {
        Self::get_class()
    }
}
