//! Small standard library for JS featuring IO and other useful stuff.
use crate::prelude::*;
use crate::vm::{object::JsObject, Runtime};
pub mod file;

/// Initialize JS std.
pub fn init_js_std(rt: &mut Runtime, mut module: GcPointer<JsObject>) -> Result<(), JsValue> {
    rt.heap().defer();
    let mut std = JsObject::new_empty(rt);
    module.put(rt, "@exports".intern(), JsValue::new(std), false)?;
    module.put(rt, "@default".intern(), JsValue::new(std), false)?;
    file::std_init_file(rt, std)?;

    rt.heap().undefer();
    Ok(())
}
