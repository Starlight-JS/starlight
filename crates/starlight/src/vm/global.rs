use std::collections::HashMap;

use wtf_rs::segmented_vec::SegmentedVec;

use crate::heap::{cell::Trace, SlotVisitor};

use super::{property_descriptor::StoredSlot, symbol_table::Symbol, Runtime};

pub struct JsGlobal {
    sym_map: HashMap<Symbol, u32>,
    variables: SegmentedVec<StoredSlot>,
    vm: *mut Runtime,
}

unsafe impl Trace for JsGlobal {
    fn trace(&self, visitor: &mut SlotVisitor) {
        self.variables.iter().for_each(|var| var.trace(visitor));
    }
}
