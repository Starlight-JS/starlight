use std::collections::HashMap;

use crate::heap::Heap;
use lasso::{LargeSpur, Rodeo};

use super::{js_symbol::JSSymbol, ref_ptr::Ref};
pub struct JSVirtualMachine {
    pub(crate) heap: Heap,
    pub(crate) interner: Rodeo<LargeSpur>,
    pub(crate) symbols: HashMap<LargeSpur, Ref<JSSymbol>>,
}

impl JSVirtualMachine {
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
