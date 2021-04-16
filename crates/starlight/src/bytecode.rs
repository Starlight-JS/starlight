use crate::{
    gc::cell::{GcPointer, Trace, Tracer, WeakRef},
    vm::{structure::Structure, structure_chain::StructureChain},
};

pub mod opcodes;
pub mod opcodes_v2;
pub mod profile;

pub enum ObservedType {
    Number,
}

pub enum TypeFeedBack {
    StructureCache {
        structure: GcPointer<Structure>,
    },
    PropertyCache {
        structure: WeakRef<Structure>,
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
