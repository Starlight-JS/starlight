use crate::{
    heap::{
        cell::{GcPointer, Trace, WeakRef},
        SlotVisitor,
    },
    vm::structure::Structure,
};

pub mod opcodes;
pub mod profile;

pub enum ObservedType {
    Number,
}

pub enum TypeFeedBack {
    PropertyCache {
        structure: WeakRef<Structure>,
        offset: u32,
    },
    None,
}

unsafe impl Trace for TypeFeedBack {
    fn trace(&self, visitor: &mut SlotVisitor) {
        match self {
            Self::PropertyCache { structure, .. } => structure.trace(visitor),
            _ => (),
        }
    }
}
