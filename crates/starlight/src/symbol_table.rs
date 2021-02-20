use std::collections::HashSet;

use crate::runtime::symbol::Symbol;

pub struct SymbolTable {
    set: HashSet<&'static str>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn lookup(&mut self, s: impl AsRef<str>) -> Symbol {
        let s = s.as_ref();
        if let Ok(uint) = s.parse::<u32>() {
            return Symbol::Indexed(uint);
        }
        let s: &'static str = Box::leak(s.to_string().into_boxed_str());
        if let Some(val) = self.set.get(&s) {
            Symbol::Key(*val)
        } else {
            let str = s.to_string();
            let s: &'static str = Box::leak(s.to_string().into_boxed_str());
            let val = s;
            std::mem::forget(str);
            self.set.insert(val);
            Symbol::Key(val)
        }
    }
}

impl Drop for SymbolTable {
    fn drop(&mut self) {
        self.set.retain(|key| {
            unsafe {
                String::from_raw_parts(key.as_ptr() as *mut u8, key.len(), key.len());
            }
            false
        });
    }
}
