use crate::gc::handle::Handle;

use super::{context::Context, js_value::JsValue};
#[allow(dead_code)]
pub struct Arguments {
    context: Handle<Context>,
    stack: *mut JsValue,
    size: usize,
    ctor_call: bool,
}
