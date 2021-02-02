use super::{context::Context, js_value::JsValue};

pub struct Arguments {
    context: super::ref_ptr::Ref<Context>,
    stack: *mut JsValue,
    size: usize,
    ctor_call: bool,
}
