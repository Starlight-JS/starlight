use crate::gc::Address;

use super::block::{HeapBlock, BLOCK_SIZE};
use std::{num::NonZeroU16, ptr::null_mut};

pub struct BlockAllocator {
    pub cache: Vec<*mut HeapBlock>,
}
/// 64 heap blocks are cached. (1MB)
pub const BLOCK_CACHE: usize = 64;
impl BlockAllocator {
    pub fn new() -> Self {
        Self {
            cache: Vec::with_capacity(64),
        }
    }

    pub fn allocate(&mut self, cell_size: NonZeroU16) -> &'static mut HeapBlock {
        match self.cache.pop() {
            Some(block) => unsafe { HeapBlock::new(block.cast(), cell_size) },
            None => unsafe {
                let mem = super::super::os::commit(BLOCK_SIZE, false).to_mut_ptr::<u8>();

                HeapBlock::new(mem, cell_size)
            },
        }
    }

    pub fn free(&mut self, block: *mut HeapBlock) {
        unsafe {
            (*block).heap = null_mut();
            (*block).freelist = null_mut();
            (*block).next_lazy_freelist_index = u16::MAX;
        }
        if self.cache.len() >= 64 {
            unsafe {
                crate::gc::os::free(Address::from_ptr(block), BLOCK_SIZE);
            }
        } else {
            self.cache.push(block);
        }
    }
}
