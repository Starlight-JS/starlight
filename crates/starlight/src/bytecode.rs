use crate::{
    gc::cell::{GcPointer, Trace, Tracer, WeakRef},
    vm::structure::Structure,
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
    None,
}

unsafe impl Trace for TypeFeedBack {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        match self {
            Self::PropertyCache { structure, .. } => structure.trace(visitor),
            Self::StructureCache { structure } => structure.trace(visitor),
            _ => (),
        }
    }
}
