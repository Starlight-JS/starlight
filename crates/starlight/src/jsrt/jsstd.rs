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
    def_native_method!(rt, std, args, std_args, 0)?;
    rt.heap().undefer();
    Ok(())
}

pub fn std_args(rt: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    let args = std::env::args()
        .map(|x| JsValue::new(JsString::new(rt, x)))
        .collect::<Vec<_>>();
    Ok(JsValue::new(JsArray::from_slice(rt, &args)))
}
