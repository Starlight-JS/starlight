use super::value::JsValue;
use super::{symbol_table::Symbol, Runtime};
use crate::heap::{cell::GcPointer, SlotVisitor};
use crate::{
    bytecode::TypeFeedBack,
    heap::cell::{GcCell, Trace},
    heap::snapshot::deserializer::Deserializable,
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

impl CodeBlock {
    pub fn new(rt: &mut Runtime, name: Symbol, strict: bool) -> GcPointer<Self> {
        let this = Self {
            name,
            strict,
            variables: vec![],
            rest_param: None,
            params: vec![],
            names: vec![],
            code: vec![],
            literals: vec![],
            feedback: vec![],
        };

        rt.heap().allocate(this)
    }
}

impl GcCell for CodeBlock {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    vtable_impl!();
}
