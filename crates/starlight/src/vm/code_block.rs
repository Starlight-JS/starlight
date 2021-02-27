use super::symbol_table::Symbol;
use super::value::JsValue;
use crate::heap::cell::{GcCell, Trace};
use crate::heap::SlotVisitor;
use starlight_derive::GcTrace;
#[derive(GcTrace)]
pub struct CodeBlock {
    pub name: Symbol,
    pub variables: Vec<Symbol>,
    pub rest_param: Option<Symbol>,
    pub params: Vec<Symbol>,
    pub names: Vec<Symbol>,
    pub code: Vec<u8>,
    pub literals: Vec<JsValue>,
    pub strict: bool,
}

impl GcCell for CodeBlock {}
