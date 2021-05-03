/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::cell::{GcPointer, Trace, Tracer},
    vm::{structure::Structure, structure_chain::StructureChain},
};

pub mod opcodes;
pub mod profile;

pub enum TypeFeedBack {
    StructureCache {
        structure: GcPointer<Structure>,
    },
    PropertyCache {
        structure: GcPointer<Structure>,
        offset: u32,
    },
    PutByIdFeedBack {
        new_structure: Option<GcPointer<Structure>>,
        old_structure: Option<GcPointer<Structure>>,
        offset: u32,
        structure_chain: Option<GcPointer<StructureChain>>,
    },
    None,
}

unsafe impl Trace for TypeFeedBack {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        match self {
            Self::PropertyCache { structure, .. } => structure.trace(visitor),
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
