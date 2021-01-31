use std::{collections::HashMap, intrinsics::drop_in_place};

use crate::heap::Heap;
use lasso::{Capacity, LargeSpur, Rodeo};

use super::{js_symbol::JSSymbol, options::Options, ref_ptr::Ref};
pub struct JSVirtualMachine {
    pub(crate) heap: Ref<Heap>,
    pub(crate) interner: Rodeo<LargeSpur>,
    pub(crate) symbols: HashMap<LargeSpur, Ref<JSSymbol>>,
    pub(crate) options: Options,
}

impl JSVirtualMachine {
    pub fn create(options: Options) -> Ref<Self> {
        let mut vm = Ref::new(Box::into_raw(Box::new(Self {
            heap: Ref::new(0 as *mut _),
            interner: Rodeo::with_capacity(Capacity::for_strings(16)),
            symbols: HashMap::new(),
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

    pub fn symbol_for(&mut self, key: impl AsRef<str>) -> Ref<JSSymbol> {
        let key = self.interner.get_or_intern(key.as_ref());
        if let Some(symbol) = self.symbols.get(&key) {
            return *symbol;
        }
        let symbol = JSSymbol::from_interned_key(self, key);
        self.symbols.insert(key, symbol);
        symbol
    }
}
