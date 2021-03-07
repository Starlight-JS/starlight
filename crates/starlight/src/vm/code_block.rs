use super::symbol_table::Symbol;
use super::value::JsValue;
use crate::heap::SlotVisitor;
use crate::{
    bytecode::TypeFeedBack,
    heap::cell::{GcCell, Trace},
};
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
    pub feedback: Vec<TypeFeedBack>,
    pub strict: bool,
}

impl GcCell for CodeBlock {}
