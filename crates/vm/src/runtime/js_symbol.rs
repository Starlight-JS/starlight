use std::mem::size_of;

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

use super::{
    js_cell::{allocate_cell, JsCell},
    symbol::Symbol,
    vm::JsVirtualMachine,
};

pub struct JsSymbol {
    sym: Symbol,
}

impl JsSymbol {
    pub fn new(vm: &mut JsVirtualMachine, sym: Symbol) -> Handle<Self> {
        let proto = Self { sym };
        allocate_cell(vm, size_of::<Self>(), proto)
    }

    pub fn sym(&self) -> Symbol {
        self.sym
    }
}

impl JsCell for JsSymbol {}

impl HeapObject for JsSymbol {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        false
    }
}

pub trait JSSymbolBuild<T> {
    fn make(vm: &mut JsVirtualMachine, key: T) -> Handle<JsSymbol>;
}

impl JSSymbolBuild<i32> for JsSymbol {
    fn make(vm: &mut JsVirtualMachine, key: i32) -> Handle<JsSymbol> {
        let k = vm.intern_i32(key);
        JsSymbol::new(vm, k)
    }
}
impl JSSymbolBuild<i64> for JsSymbol {
    fn make(vm: &mut JsVirtualMachine, key: i64) -> Handle<JsSymbol> {
        let k = vm.intern_i64(key);
        JsSymbol::new(vm, k)
    }
}

impl JSSymbolBuild<u32> for JsSymbol {
    fn make(vm: &mut JsVirtualMachine, key: u32) -> Handle<JsSymbol> {
        let k = vm.intern_u32(key);
        JsSymbol::new(vm, k)
    }
}

impl JSSymbolBuild<f64> for JsSymbol {
    fn make(vm: &mut JsVirtualMachine, key: f64) -> Handle<JsSymbol> {
        let k = vm.intern_f64(key);
        JsSymbol::new(vm, k)
    }
}

impl JSSymbolBuild<&str> for JsSymbol {
    fn make(vm: &mut JsVirtualMachine, key: &str) -> Handle<JsSymbol> {
        let k = vm.intern(key);
        JsSymbol::new(vm, k)
    }
}
