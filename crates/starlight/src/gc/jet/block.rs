use std::{mem::size_of, num::NonZeroU16, ptr::null_mut};

use crate::gc::cell::GcPointerBase;

use super::{JetGC, JET_FREE};

pub const BLOCK_SIZE: usize = 16 * 1024;

#[repr(C)]
pub struct HeapBlock {
    pub next: *mut Self,
    pub heap: *mut JetGC,
    pub cell_size: NonZeroU16,
    pub next_lazy_freelist_index: u16,
    pub freelist: *mut FreeEntry,
    pub storage: [u16; 0],
}

impl HeapBlock {
    pub fn cell(&self, index: usize) -> *mut u8 {
        unsafe {
            let ptr = self.storage.as_ptr() as *mut u8;
            ptr.add(index * self.cell_size.get() as usize)
        }
    }

    pub fn has_lazy_freelist(&self) -> bool {
        self.next_lazy_freelist_index < self.cell_count() as u16
    }

    pub fn cell_size(&self) -> usize {
        self.cell_size.get() as _
    }

    pub fn for_each_cell(&self, mut cb: impl FnMut(*mut u8)) {
        let end = if self.has_lazy_freelist() {
            self.next_lazy_freelist_index
        } else {
            self.cell_count() as u16
        };
        for i in 0..end {
            cb(self.cell(i as _));
        }
    }

    pub fn from_cell(cell: *const u8) -> *mut Self {
        ((cell as usize) & !(BLOCK_SIZE - 1)) as _
    }

    pub fn cell_from_possible_pointer(&self, pointer: *const u8) -> *mut u8 {
        if pointer < self.storage.as_ptr() as *const u8 {
            return null_mut();
        }
        let cell_index = (pointer as usize - self.storage.as_ptr() as usize) / self.cell_size();
        let end = if self.has_lazy_freelist() {
            self.next_lazy_freelist_index as usize
        } else {
            self.cell_count()
        };
        if cell_index > end {
            return null_mut();
        }
        self.cell(cell_index)
    }

    pub fn is_valid_cell_pointer(&self, cell: *const u8) -> bool {
        !self.cell_from_possible_pointer(cell).is_null()
    }
    pub fn cell_count(&self) -> usize {
        (BLOCK_SIZE - size_of::<Self>()) / self.cell_size.get() as usize
    }

    pub fn allocate(&mut self) -> *mut u8 {
        if !self.freelist.is_null() {
            unsafe {
                let cell = self.freelist;
                self.freelist = (*cell).next;
                return cell.cast();
            }
        } else if self.has_lazy_freelist() {
            let cell = self.cell(self.next_lazy_freelist_index as usize);
            self.next_lazy_freelist_index += 1;
            return cell;
        }
        null_mut()
    }

    pub fn new(at: *mut u8, cell_size: NonZeroU16) -> &'static mut Self {
        unsafe {
            at.cast::<Self>().write(Self {
                heap: null_mut(),
                cell_size,
                freelist: null_mut(),
                next_lazy_freelist_index: 0,
                next: null_mut(),
                storage: [],
            });
            let mut freelist = null_mut();
            (*at.cast::<Self>()).for_each_cell(|cell| {
                (*cell.cast::<GcPointerBase>()).force_set_state(JET_FREE);
                (*cell.cast::<FreeEntry>()).next = freelist;
                freelist = cell.cast();
            });
            (*at.cast::<Self>()).freelist = freelist;
            &mut *at.cast()
        }
    }

    pub fn is_full(&self) -> bool {
        !self.has_lazy_freelist() && self.freelist.is_null()
    }
}

#[repr(C)]
pub struct FreeEntry {
    pub dummy: u64,
    pub next: *mut Self,
}
