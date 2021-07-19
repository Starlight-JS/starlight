use starlight::{prelude::*, vm::context::Context};

pub fn _262_create_realm(ctx: GcPointer<Context>, _: &Arguments) -> Result<JsValue, JsValue> {
    let new_ctx = ctx.vm().new_context();
    init(new_ctx).map(JsValue::new)
}

pub fn init(mut ctx: GcPointer<Context>) -> Result<GcPointer<JsObject>, JsValue> {
    let mut global_object = ctx.global_object();

    let mut object = JsObject::new_empty(ctx);
    let fun = JsNativeFunction::new(ctx, "createRealm".intern(), _262_create_realm, 0);
    object.put(ctx, "createRealm".intern(), JsValue::new(fun), false)?;
    let ctx2 = ctx;
    let eval_script = JsClosureFunction::new(
        ctx2,
        "evalScript".intern(),
        move |_ctx, args| {
            let mut rctx = ctx;
            if let Ok(source) = args.at(0).to_string(_ctx) {
                rctx.eval(&source)
            } else {
                Ok(JsValue::encode_undefined_value())
            }
        },
        1,
    );
    object.put(ctx, "evalScript".intern(), JsValue::new(eval_script), false)?;
    let gc = JsNativeFunction::new(ctx, "gc".intern(), starlight::jsrt::global::gc, 0);
    object.put(ctx, "gc".intern(), JsValue::new(gc), false)?;
    object.put(ctx, "global".intern(), JsValue::new(global_object), false)?;
    global_object.put(ctx, "$262".intern(), JsValue::new(object), false)?;

    Ok(object)
}
