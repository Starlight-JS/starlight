//! Module that contains builtin function definition. These functions is not exposed to JavaScript in any way.
//!
//!
//! Builtins is used most of the times as "slow path"s for regular operations (i.e call with spread parameter will invoke `reflect_apply`)
//!
//!

use super::value::*;
use super::{interpreter::frame::CallFrame, Runtime};

pub fn reflect_apply(
    rt: &mut Runtime,
    frame: &mut CallFrame,
    ip: &mut *mut u8,
) -> Result<JsValue, JsValue> {
    todo!()
}
