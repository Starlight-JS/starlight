use std::{ptr::null_mut, usize};

use wtf_rs::object_offsetof;

use super::heap_cell::HeapCell;

pub struct FreeList {
    scrambled_head: usize,
    secret: usize,
    payload_end: *mut u8,
    remaining: u32,
    original_size: u32,
    cell_size: u32,
}

impl FreeList {
    pub fn offset_of_scrambled_head() -> usize {
        object_offsetof!(FreeList, scrambled_head)
    }

    pub fn offset_of_secret() -> usize {
        object_offsetof!(FreeList, secret)
    }

    pub fn offset_of_payload_end() -> usize {
        object_offsetof!(FreeList, payload_end)
    }

    pub fn offset_of_remaining() -> usize {
        object_offsetof!(FreeList, remaining)
    }

    pub fn offset_of_original_size() -> usize {
        object_offsetof!(FreeList, original_size)
    }
    pub fn offset_of_cell_size() -> usize {
        object_offsetof!(FreeList, cell_size)
    }

    pub fn original_size(&self) -> u32 {
        self.original_size
    }
    pub fn cell_size(&self) -> u32 {
        self.cell_size
    }
    pub const fn new(cell_size: u32) -> Self {
        Self {
            cell_size,
            remaining: 0,
            original_size: 0,
            payload_end: null_mut(),
            secret: 0,
            scrambled_head: 0,
        }
    }

    pub fn clear(&mut self) {
        self.secret = 0;
        self.remaining = 0;
        self.original_size = 0;
        self.scrambled_head = 0;
        self.payload_end = 0 as _;
    }
    pub fn initialize_list(&mut self, head: *mut FreeCell, secret: usize, bytes: u32) {
        self.scrambled_head = FreeCell::scramble(head, secret);
        self.secret = secret;
        self.payload_end = 0 as _;
        self.remaining = 0;
        self.original_size = bytes;
    }

    pub fn initialize_bump(&mut self, payload_end: *mut u8, remaining: u32) {
        self.scrambled_head = 0;
        self.secret = 0;
        self.payload_end = payload_end;
        self.remaining = remaining;
        self.original_size = remaining;
    }

    pub fn contains(&self, target: *const HeapCell) -> bool {
        if self.remaining != 0 {
            let start = self.payload_end as usize - self.remaining as usize;
            let end = self.payload_end as usize;
            return (start <= target as usize) && ((target as usize) < end);
        }

        let mut candidate = self.head();
        unsafe {
            while !candidate.is_null() {
                if candidate as usize == target as usize {
                    return true;
                }
                candidate = (*candidate).next(self.secret);
            }
        }
        false
    }
    fn head(&self) -> *mut FreeCell {
        FreeCell::descramble(self.scrambled_head, self.secret)
    }

    pub fn allocate(&mut self, mut slow_path: impl FnMut() -> *mut HeapCell) -> *mut HeapCell {
        let mut remaining = self.remaining;
        if remaining != 0 {
            remaining -= self.cell_size;
            self.remaining = remaining;
            return (self.payload_end as usize - remaining as usize - self.cell_size as usize) as _;
        }

        let result = self.head();
        if result.is_null() {
            return slow_path();
        }
        self.scrambled_head = unsafe { (*result).scrambled_next };
        result as _
    }
}

#[repr(C)]
pub struct FreeCell {
    pub preserved: u64,
    pub scrambled_next: usize,
}

impl FreeCell {
    pub fn scramble(cell: *mut FreeCell, secret: usize) -> usize {
        cell as usize ^ secret
    }

    pub fn descramble(cell: usize, secret: usize) -> *mut FreeCell {
        (cell ^ secret) as _
    }

    pub fn set_next(&mut self, next: *mut FreeCell, secret: usize) {
        self.scrambled_next = Self::scramble(next, secret)
    }

    pub fn next(&self, secret: usize) -> *mut FreeCell {
        Self::descramble(self.scrambled_next, secret)
    }

    pub fn offset_of_scrambled_next() -> usize {
        object_offsetof!(FreeCell, scrambled_next)
    }
}
