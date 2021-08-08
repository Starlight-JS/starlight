/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::collections::HashMap;

use crate::gc::cell::{GcCell, GcPointer, Trace, Visitor};

use super::{
    array_storage::ArrayStorage, attributes::object_data, property_descriptor::StoredSlot,
    value::JsValue, Context,
};

const FLAG_DENSE: u8 = 1;
const FLAG_WRITABLE: u8 = 2;
/// 256*n
pub const MAX_VECTOR_SIZE: usize = 1024 << 6;

pub type SparseArrayMap = HashMap<u32, StoredSlot>;
pub type DenseArrayMap = ArrayStorage;

pub struct IndexedElements {
    pub(crate) map: Option<GcPointer<SparseArrayMap>>,
    pub(crate) vector: GcPointer<DenseArrayMap>,
    pub(crate) length: u32,
    pub(crate) flags: u32,
    pub(crate) non_gc: bool,
}

impl IndexedElements {
    #[allow(clippy::explicit_counter_loop)]
    pub fn make_sparse(&mut self, ctx: GcPointer<Context>) {
        self.flags &= !(FLAG_DENSE as u32);
        let mut sparse = self.ensure_map(ctx);
        let mut index = 0;
        for i in 0..self.vector.size() {
            if !self.vector.at(i).is_empty() {
                sparse.insert(
                    index,
                    StoredSlot::new_raw(*self.vector.at(i), object_data()),
                );
            }
            index += 1;
        }
        for i in 0..self.vector.size() {
            *self.vector.at_mut(i) = JsValue::encode_empty_value();
        }
    }

    pub fn make_dense(&mut self) {
        self.flags |= FLAG_DENSE as u32;
        self.map = None;
    }

    pub fn ensure_map(&mut self, mut ctx: GcPointer<Context>) -> GcPointer<SparseArrayMap> {
        match self.map.as_ref() {
            Some(map) => *map,
            None => {
                let map = ctx.heap().allocate(HashMap::with_capacity(8));
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

    pub fn new(mut ctx: GcPointer<Context>) -> Self {
        Self {
            length: 0,
            flags: FLAG_DENSE as u32 | FLAG_WRITABLE as u32,
            vector: ArrayStorage::new(ctx.heap(), 0),
            map: None,
            non_gc: true,
        }
    }
}

impl Trace for IndexedElements {
    fn trace(&self, visitor: &mut Visitor) {
        self.vector.trace(visitor);
        self.map.trace(visitor);
    }
}
impl GcCell for IndexedElements {}
