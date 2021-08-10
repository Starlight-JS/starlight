/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::cell::{GcPointer, Trace, Visitor},
    vm::{object::JsObject, structure::Structure, structure_chain::StructureChain},
};

pub mod block;
pub mod opcodes;
pub mod profile;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GetByIdMode {
    Default,
    ProtoLoad(GcPointer<JsObject> /*cached slot */),
    ArrayLength,
}
pub enum TypeFeedBack {
    StructureCache {
        structure: GcPointer<Structure>,
    },
    PropertyCache {
        structure: GcPointer<Structure>,
        offset: u32,
        mode: GetByIdMode,
    },
    PutByIdFeedBack {
        new_structure: Option<GcPointer<Structure>>,
        old_structure: Option<GcPointer<Structure>>,
        offset: u32,
        structure_chain: Option<GcPointer<StructureChain>>,
    },
    None,
}

impl Trace for TypeFeedBack {
    fn trace(&self, visitor: &mut Visitor) {
        match self {
            Self::PropertyCache {
                structure, mode, ..
            } => {
                structure.trace(visitor);
                match mode {
                    GetByIdMode::ProtoLoad(slot) => slot.trace(visitor),
                    _ => (),
                }
            }
            Self::StructureCache { structure } => structure.trace(visitor),
            Self::PutByIdFeedBack {
                new_structure,
                old_structure,
                structure_chain,
                ..
            } => {
                new_structure.trace(visitor);
                old_structure.trace(visitor);
                structure_chain.trace(visitor);
            }
            _ => (),
        }
    }
}
