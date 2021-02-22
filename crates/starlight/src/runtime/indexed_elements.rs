use std::collections::HashMap;

use super::{
    attributes::object_data, gc_array::GcVec, property_descriptor::StoredSlot, value::JsValue,
};
use crate::{
    gc::cell::{Cell, Gc, Trace, Tracer},
    vm::VirtualMachine,
};
use minivec::MiniVec;
const FLAG_DENSE: u8 = 1;
const FLAG_WRITABLE: u8 = 2;
/// 256*n
pub const MAX_VECTOR_SIZE: usize = 1024 << 6;
pub type SparseArrayMap = HashMap<u32, StoredSlot>;
pub type DenseArrayVector = MiniVec<JsValue>;

#[repr(C)]
pub struct IndexedElements {
    pub(crate) map: Option<Gc<SparseArrayMap>>,
    pub(crate) vector: GcVec<JsValue>,
    length: u32,
    flags: u32,
}

impl IndexedElements {
    #[allow(clippy::explicit_counter_loop)]
    pub fn make_sparse(&mut self, vm: &mut VirtualMachine) {
        self.flags &= !(FLAG_DENSE as u32);
        let mut sparse = self.ensure_map(vm);
        let mut index = 0;
        for i in 0..self.vector.len() {
            if !self.vector[i].is_empty() {
                sparse.insert(index, StoredSlot::new_raw(self.vector[i], object_data()));
            }
            index += 1;
        }
        for i in 0..self.vector.len() {
            self.vector[i] = JsValue::empty();
        }
    }

    pub fn make_dense(&mut self) {
        self.flags |= FLAG_DENSE as u32;
        self.map = None;
    }

    pub fn ensure_map(&mut self, vm: &mut VirtualMachine) -> Gc<SparseArrayMap> {
        match self.map {
            Some(map) => map,
            None => {
                let map = vm.space().alloc(HashMap::with_capacity(8));
                self.map = Some(map);
                map
            }
        }
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn set_length(&mut self, len: u32) {
        self.length = len;
    }

    pub fn dense(&self) -> bool {
        (self.flags & FLAG_DENSE as u32) != 0
    }

    pub fn sparse(&self) -> bool {
        !self.dense()
    }

    pub fn writable(&self) -> bool {
        (self.flags & FLAG_WRITABLE as u32) != 0
    }

    pub fn make_readonly(&mut self) {
        self.flags &= !(FLAG_WRITABLE as u32);
    }

    pub fn new(_vm: &mut VirtualMachine) -> Self {
        Self {
            length: 0,
            flags: FLAG_DENSE as u32 | FLAG_WRITABLE as u32,
            vector: GcVec::new(_vm, 0),
            map: None,
        }
    }
}

impl Cell for IndexedElements {}
unsafe impl Trace for IndexedElements {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.map.trace(tracer);
        /*for item in self.vector.iter() {
            item.trace(tracer);
        }*/
        self.vector.trace(tracer);
    }
}

impl Cell for StoredSlot {}
unsafe impl Trace for StoredSlot {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.value.trace(tracer);
    }
}
