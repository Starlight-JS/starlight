use std::mem::size_of;

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

use super::{
    js_cell::{allocate_cell, JsCell},
    ref_ptr::*,
    symbol::Symbol,
    vm::JsVirtualMachine,
};

pub struct JsSymbol {
    sym: Symbol,
}

impl JsSymbol {
    pub fn new(vm: Ref<JsVirtualMachine>, sym: Symbol) -> Handle<Self> {
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
    fn make(vm: Ref<JsVirtualMachine>, key: T) -> Handle<JsSymbol>;
}

impl JSSymbolBuild<i32> for JsSymbol {
    fn make(mut vm: Ref<JsVirtualMachine>, key: i32) -> Handle<JsSymbol> {
        JsSymbol::new(vm, vm.intern_i32(key))
    }
}
impl JSSymbolBuild<i64> for JsSymbol {
    fn make(mut vm: Ref<JsVirtualMachine>, key: i64) -> Handle<JsSymbol> {
        JsSymbol::new(vm, vm.intern_i64(key))
    }
}

impl JSSymbolBuild<u32> for JsSymbol {
    fn make(mut vm: Ref<JsVirtualMachine>, key: u32) -> Handle<JsSymbol> {
        JsSymbol::new(vm, vm.intern_u32(key))
    }
}

impl JSSymbolBuild<f64> for JsSymbol {
    fn make(mut vm: Ref<JsVirtualMachine>, key: f64) -> Handle<JsSymbol> {
        JsSymbol::new(vm, vm.intern_f64(key))
    }
}

impl JSSymbolBuild<&str> for JsSymbol {
    fn make(mut vm: Ref<JsVirtualMachine>, key: &str) -> Handle<JsSymbol> {
        JsSymbol::new(vm, vm.intern(key))
    }
}
