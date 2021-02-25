use crate::heap::cell::{GcCell, Trace};
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EnumerationMode {
    Default,
    IncludeNotEnumerable,
}
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum JsHint {
    String,
    Number,
    None,
}
pub struct JsObject {}

unsafe impl Trace for JsObject {}
impl GcCell for JsObject {}
