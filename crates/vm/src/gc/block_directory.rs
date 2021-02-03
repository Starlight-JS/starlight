#![allow(dead_code)]
use super::marked_block::Handle;

pub struct BlockDirectory {
    blocks: Vec<*mut Handle>,
    free_blocks: Vec<*mut Handle>,
    recyclable_blocks: Vec<*mut Handle>,
    unavailable_blocks: Vec<*mut Handle>,
    cell_size: u32,
}

impl BlockDirectory {
    pub fn new(cell_size: u32) -> Self {
        Self {
            blocks: vec![],
            free_blocks: vec![],
            recyclable_blocks: vec![],
            unavailable_blocks: vec![],
            cell_size,
        }
    }

    pub fn find_empty_block_to_steal(&mut self) -> *mut Handle {
        self.free_blocks.pop().unwrap_or(core::ptr::null_mut())
    }
}
