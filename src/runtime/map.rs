use std::{collections::HashMap, mem::size_of};

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

use super::{
    attributes::AttrSafe,
    js_cell::{allocate_cell, JsCell},
    ref_ptr::Ref,
    symbol::Symbol,
    vm::JsVirtualMachine,
};

/// Map object is like object
/// These structures are used for implementing Polymorphic Inline Cache.
///
///
/// original paper is
///   http://cs.au.dk/~hosc/local/LaSC-4-3-pp243-281.pdf
///

pub struct Map {
    transitions: Transitions,
}

pub struct MapEntry {
    offset: u32,
    attrs: AttrSafe,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransitionKey {
    name: Symbol,
    attrs: u32,
}

impl JsCell for TransitionKey {}
impl HeapObject for TransitionKey {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        false
    }
}

union U {
    table: Option<Handle<Table>>,
    pair: (TransitionKey, Option<Handle<Map>>),
}
pub struct Transitions {
    u: U,
    flags: u8,
}

const MASK_ENABLED: u8 = 1;
const MASK_UNIQUE_TRANSITION: u8 = 2;
const MASK_HOLD_SINGLE: u8 = 4;
const MASK_HOLD_TABLE: u8 = 8;
const MASK_INDEXED: u8 = 16;

type Table = HashMap<TransitionKey, Option<Handle<Map>>>;

impl Transitions {
    pub fn set_indexed(&mut self, indexed: bool) {
        if indexed {
            self.flags |= MASK_INDEXED;
        } else {
            self.flags &= !MASK_INDEXED;
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.flags |= MASK_ENABLED;
        } else {
            self.flags &= !MASK_ENABLED;
        }
    }

    pub fn is_enabled_unique_transition(&self) -> bool {
        (self.flags & MASK_UNIQUE_TRANSITION) != 0
    }

    pub fn enable_unique_transition(&mut self) {
        self.flags |= MASK_UNIQUE_TRANSITION;
    }

    pub fn insert(
        &mut self,
        vm: Ref<JsVirtualMachine>,
        name: Symbol,
        attrs: AttrSafe,
        map: Handle<Map>,
    ) {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        unsafe {
            if (self.flags & MASK_HOLD_SINGLE) != 0 {
                let mut table: Handle<Table> =
                    allocate_cell(vm, size_of::<Table>(), Default::default());
                table.insert(self.u.pair.0, self.u.pair.1);
                self.u.table = Some(table);
                self.flags &= !MASK_HOLD_SINGLE;
                self.flags &= MASK_HOLD_TABLE;
            }
            if (self.flags & MASK_HOLD_TABLE) != 0 {
                self.u.table.unwrap().insert(key, Some(map));
            } else {
                self.u.pair.0 = key;
                self.u.pair.1 = Some(map);
                self.flags |= MASK_HOLD_SINGLE;
            }
        }
    }

    pub fn find(&self, name: Symbol, attrs: AttrSafe) -> Option<Handle<Map>> {
        let key = TransitionKey {
            name,
            attrs: attrs.raw(),
        };
        unsafe {
            if (self.flags & MASK_HOLD_TABLE) != 0 {
                return self.u.table.unwrap().get(&key).copied().flatten();
            } else if (self.flags & MASK_HOLD_SINGLE) != 0 {
                if self.u.pair.0 == key {
                    return self.u.pair.1;
                }
            }
        }
        None
    }

    pub fn is_enabled(&self) -> bool {
        (self.flags & MASK_ENABLED) != 0
    }

    pub fn is_indexed(&self) -> bool {
        (self.flags & MASK_INDEXED) != 0
    }
}

impl JsCell for Map {}
impl HeapObject for Map {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        unsafe {
            if (self.transitions.flags & MASK_HOLD_SINGLE) != 0 {
                if let Some(ref mut map) = self.transitions.u.pair.1 {
                    map.visit_children(tracer);
                }
            } else if (self.transitions.flags & MASK_HOLD_TABLE) != 0 {
                if let Some(ref mut table) = self.transitions.u.table {
                    table.visit_children(tracer);
                }
            }
        }
    }

    fn needs_destruction(&self) -> bool {
        true
    }
}
