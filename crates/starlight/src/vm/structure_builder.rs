use super::{
    attributes::AttrSafe,
    context::Context,
    object::JsObject,
    structure::{MapEntry, Structure, TargetTable},
    symbol_table::Symbol,
};
use crate::gc::cell::GcPointer;
pub struct StructureBuilder {
    target_table: TargetTable,
    prototype: Option<GcPointer<JsObject>>,
}

impl StructureBuilder {
    pub fn new(prototype: Option<GcPointer<JsObject>>) -> Self {
        Self {
            target_table: TargetTable::new(),
            prototype,
        }
    }

    pub fn build(
        self,
        ctx: GcPointer<Context>,
        unique: bool,
        indexed: bool,
    ) -> GcPointer<Structure> {
        Structure::new_from_table(
            ctx,
            Some(self.target_table),
            self.prototype,
            unique,
            indexed,
        )
    }

    pub fn add_at(&mut self, symbol: Symbol, index: usize, attributes: AttrSafe) -> MapEntry {
        assert!(self.find(symbol).is_none());
        self.target_table.insert(
            symbol,
            MapEntry {
                attrs: attributes,
                offset: index as _,
            },
        );
        MapEntry {
            attrs: attributes,
            offset: index as _,
        }
    }

    pub fn add(&mut self, symbol: Symbol, attributes: AttrSafe) -> MapEntry {
        let index = self.target_table.len();
        self.add_at(symbol, index, attributes)
    }

    pub fn override_(&mut self, symbol: Symbol, entry: MapEntry) {
        self.target_table
            .insert(symbol, entry)
            .expect("Property does not exist");
    }

    pub fn find(&self, symbol: Symbol) -> Option<MapEntry> {
        self.target_table.get(&symbol).copied()
    }
}
