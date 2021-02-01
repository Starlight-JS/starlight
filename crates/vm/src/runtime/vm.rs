use std::intrinsics::drop_in_place;

use super::symbol::Symbol;
use super::{options::Options, ref_ptr::Ref, symbol_table::SymbolTable};
use crate::heap::Heap;

pub struct JsVirtualMachine {
    pub(crate) heap: Ref<Heap>,
    pub(crate) sym_table: SymbolTable,

    pub(crate) options: Options,
}

impl JsVirtualMachine {
    pub fn create(options: Options) -> Ref<Self> {
        let mut vm = Ref::new(Box::into_raw(Box::new(Self {
            heap: Ref::new(0 as *mut _),
            sym_table: SymbolTable::new(),

            options,
        })));
        vm.heap = Ref::new(Box::into_raw(Box::new(Heap::new(
            vm,
            vm.options.heap_size,
            vm.options.threshold,
        ))));

        vm
    }
    pub fn gc(&mut self, evac: bool) {
        self.heap.gc(evac)
    }
    pub fn dispose(vm: Ref<Self>) {
        unsafe {
            drop_in_place(vm.pointer);
        }
    }

    pub fn intern(&mut self, key: impl AsRef<str>) -> Symbol {
        self.sym_table.intern(key)
    }

    pub fn intern_i32(&mut self, key: i32) -> Symbol {
        let converted = key as u32;
        if converted as i32 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }

    pub fn intern_i64(&mut self, key: i64) -> Symbol {
        let converted = key as u32;
        if converted as i64 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }

    pub fn intern_u32(&mut self, key: u32) -> Symbol {
        Symbol::Indexed(key)
    }

    pub fn intern_f64(&mut self, key: f64) -> Symbol {
        let converted = key as u32;
        if converted as f64 == key {
            return Symbol::Indexed(converted);
        }
        self.intern(key.to_string())
    }
}
