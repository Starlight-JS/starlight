use std::collections::HashMap;

const FLAG_DENSE: u8 = 1;
const FLAG_WRITABLE: u8 = 2;

pub type SparseArrayMap = HashMap<u32, ()>;

pub struct IndexedElements {}
