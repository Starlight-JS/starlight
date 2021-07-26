use crate::prelude::*;
use crate::vm::context::*;

pub fn _262_create_realm(ctx: GcPointer<Context>, _: &Arguments) -> Result<JsValue, JsValue> {
    let new_ctx = ctx.vm().new_context();
    init(new_ctx, "$".intern()).map(JsValue::new)
}
pub fn _262_eval_script(mut ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if let Ok(source) = args.at(0).to_string(ctx) {
        ctx.eval(&source)
    } else {
        Ok(JsValue::encode_undefined_value())
    }
}
pub fn init(mut ctx: GcPointer<Context>, as_: Symbol) -> Result<GcPointer<JsObject>, JsValue> {
    let mut global_object = ctx.global_object();

    let mut object = JsObject::new_empty(ctx);
    let fun = JsNativeFunction::new(ctx, "createRealm".intern(), _262_create_realm, 0);
    object.put(ctx, "createRealm".intern(), JsValue::new(fun), false)?;

    let eval_script = JsNativeFunction::new(ctx, "evalScript".intern(), _262_eval_script, 1);
    object.put(ctx, "evalScript".intern(), JsValue::new(eval_script), false)?;
    let gc = JsNativeFunction::new(ctx, "gc".intern(), crate::jsrt::global::gc, 0);
    object.put(ctx, "gc".intern(), JsValue::new(gc), false)?;
    object.put(ctx, "global".intern(), JsValue::new(global_object), false)?;
    global_object.put(ctx, as_, JsValue::new(object), false)?;
    object.put(ctx, "agent".intern(), JsValue::UNDEFINED, false)?;
    Ok(object)
}
