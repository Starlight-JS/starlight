//! Small standard library for JS featuring IO and other useful stuff.
use crate::prelude::*;
use crate::vm::context::Context;
use crate::vm::{object::JsObject};
pub mod file;

/// Initialize JS std.
pub fn init_js_std(ctx: &mut Context, mut module: GcPointer<JsObject>) -> Result<(), JsValue> {
    ctx.heap().defer();
    let mut std = JsObject::new_empty(ctx);
    module.put(ctx, "@expoctxs".intern(), JsValue::new(std), false)?;
    module.put(ctx, "@default".intern(), JsValue::new(std), false)?;
    file::std_init_file(ctx, std)?;
    def_native_method!(ctx, std, args, std_args, 0)?;
    ctx.heap().undefer();
    Ok(())
}

pub fn std_args(ctx: &mut Context, _args: &Arguments) -> Result<JsValue, JsValue> {
    let args = std::env::args()
        .map(|x| JsValue::new(JsString::new(ctx, x)))
        .collect::<Vec<_>>();
    Ok(JsValue::new(JsArray::from_slice(ctx, &args)))
}
