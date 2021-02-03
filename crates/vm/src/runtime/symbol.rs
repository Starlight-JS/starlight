use lasso::Spur;

use crate::{gc::heap_cell::HeapObject, heap::trace::Tracer};

use super::js_cell::JsCell;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolPublicity {
    Public,
    Private,
    Unspecified,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Key(Option<Spur>, SymbolPublicity),
    Indexed(u32),
}

impl Symbol {
    pub fn is_private(&self) -> bool {
        /*if let Symbol::Key(_, SymbolPublicity::Private) = self {
            true
        } else {
            false
        }*/
        matches!(self, Symbol::Key(_, SymbolPublicity::Private))
    }

    pub fn is_public(&self) -> bool {
        matches!(self, Symbol::Key(_, SymbolPublicity::Public)) /* {
                                                                    true
                                                                } else {
                                                                    false
                                                                }*/
    }
}

pub const DUMMY_SYMBOL: Symbol = Symbol::Key(None, SymbolPublicity::Unspecified);

impl HeapObject for Symbol {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        false
    }
}
impl JsCell for Symbol {}
