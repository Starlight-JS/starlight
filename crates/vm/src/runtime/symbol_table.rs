use std::unimplemented;

use lasso::{Rodeo, Spur};

use super::symbol::{Symbol, SymbolPublicity};

pub struct SymbolTable {
    rodeo: Rodeo<Spur>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            rodeo: Rodeo::new(),
        }
    }
    pub fn intern(&mut self, key: impl AsRef<str>) -> Symbol {
        Symbol::Key(
            Some(self.rodeo.get_or_intern(key.as_ref())),
            SymbolPublicity::Unspecified,
        )
    }

    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(Some(key), _) => self.rodeo.resolve(&key).to_owned(),
            Symbol::Indexed(index) => index.to_string(),
            _ => unimplemented!(),
        }
    }
}
