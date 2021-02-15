use crate::heap::tiny_bloom_filter::TinyBloomFilter;

use super::block::HeapBlock;
use std::collections::HashSet;

pub struct BlockSet {
    pub set: HashSet<*mut HeapBlock>,
    pub filter: TinyBloomFilter,
}

impl BlockSet {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
            filter: TinyBloomFilter::new(0),
        }
    }
    pub fn add(&mut self, block: *mut HeapBlock) {
        self.filter.add_bits(block as _);
        self.set.insert(block);
    }

    pub fn remove(&mut self, block: *mut HeapBlock) {
        //let old_cap = self.set.capacity();
        self.set.remove(&block);
        /*self.set.shrink_to_fit();
        if self.set.capacity() != old_cap {
            self.recompute_filter();
        }*/
    }

    pub fn recompute_filter(&mut self) {
        let mut filter = TinyBloomFilter::new(0);
        for block in self.set.iter() {
            let block = *block;
            filter.add_bits(block as usize);
        }
        self.filter = filter;
    }
}
