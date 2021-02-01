use lasso::Spur;

use crate::{gc::heap_cell::HeapObject, heap::trace::Tracer};

use super::js_cell::JsCell;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Key(Option<Spur>),
    Indexed(u32),
}

pub const DUMMY_SYMBOL: Symbol = Symbol::Key(None);

impl HeapObject for Symbol {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        false
    }
}
impl JsCell for Symbol {}
