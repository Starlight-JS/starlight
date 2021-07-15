use starlight::{prelude::*, vm::context::Context};

pub fn _262_create_realm(ctx: &mut Context, _args: &Arguments) -> Result<JsValue, JsValue> {
    let new_ctx = Context::new(&mut ctx.vm());
    init(new_ctx).map(JsValue::new)
}

pub fn init(mut ctx: GcPointer<Context>) -> Result<GcPointer<JsObject>, JsValue> {
    let mut global_object = ctx.global_object();

    let mut object = JsObject::new_empty(&mut ctx);
    let fun = JsNativeFunction::new(&mut ctx, "createRealm".intern(), _262_create_realm, 0);
    object.put(&mut ctx, "createRealm".intern(), JsValue::new(fun), false)?;
    let mut ctx2 = ctx;
    let eval_script = JsClosureFunction::new(
        &mut ctx2,
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
    object.put(
        &mut ctx,
        "evalScript".intern(),
        JsValue::new(eval_script),
        false,
    )?;
    let gc = JsNativeFunction::new(&mut ctx, "gc".intern(), starlight::jsrt::global::gc, 0);
    object.put(&mut ctx, "gc".intern(), JsValue::new(gc), false)?;
    object.put(
        &mut ctx,
        "global".intern(),
        JsValue::new(global_object),
        false,
    )?;
    global_object.put(&mut ctx, "$262".intern(), JsValue::new(object), false)?;

    Ok(object)
}
