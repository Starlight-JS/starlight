use crate::{
    gc::cell::{Trace, Tracer, WeakRef},
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
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        match self {
            Self::PropertyCache { structure, .. } => structure.trace(visitor),
            _ => (),
        }
    }
}
