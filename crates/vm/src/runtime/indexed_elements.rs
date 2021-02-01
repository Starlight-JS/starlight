use std::{collections::HashMap, mem::size_of};

use crate::{gc::handle::Handle, util::array::GcVec};

use super::{
    attributes::object_data, js_cell::allocate_cell, js_value::JsValue,
    property_descriptor::StoredSlot, ref_ptr::AsRefPtr, vm::JsVirtualMachine,
};

const FLAG_DENSE: u8 = 1;
const FLAG_WRITABLE: u8 = 2;
/// 256*n
pub const MAX_VECTOR_SIZE: usize = 1024 << 6;
pub type SparseArrayMap = HashMap<u32, StoredSlot>;
pub type DenseArrayVector = GcVec<JsValue>;

pub struct IndexedElements {
    map: Option<Handle<SparseArrayMap>>,
    vector: DenseArrayVector,
    length: u32,
    flags: u32,
}

impl IndexedElements {
    pub fn make_sparse(&mut self, vm: impl AsRefPtr<JsVirtualMachine>) {
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

    pub fn ensure_map(&mut self, vm: impl AsRefPtr<JsVirtualMachine>) -> Handle<SparseArrayMap> {
        match self.map {
            Some(map) => map,
            None => {
                let map = allocate_cell(vm, size_of::<SparseArrayMap>(), HashMap::with_capacity(8));
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

    pub fn new(vm: impl AsRefPtr<JsVirtualMachine>) -> Self {
        Self {
            length: 0,
            flags: FLAG_DENSE as u32 | FLAG_WRITABLE as u32,
            vector: GcVec::new(vm.as_ref_ptr(), 0),
            map: None,
        }
    }
}
