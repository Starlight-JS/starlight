use crate::heap::{
    addr::Address,
    cell::{Header, GC_UNMARKED},
};
use intrusive_collections::intrusive_adapter;
use intrusive_collections::LinkedListLink;
use intrusive_collections::UnsafeRef;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    mem::size_of,
    ptr::{null_mut, NonNull},
};

use super::heap::Heap;
intrusive_adapter!(pub BlockAdapter = UnsafeRef<HeapBlock> : HeapBlock {link: LinkedListLink});

#[repr(C)]
pub struct FreeCell {
    bytes: u64,
    next: *mut Self,
}

/// Singly linked list used as free-list
#[derive(Clone, Copy)]
pub struct FreeList {
    head: *mut FreeCell,
}

impl FreeList {
    /// Create new freelist
    pub const fn new() -> Self {
        Self {
            head: core::ptr::null_mut(),
        }
    }
    /// Is this freelist empty?
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }
    /// Try to pop from list
    pub fn allocate(&mut self, _sz: usize) -> Address {
        if self.is_empty() {
            return Address::null();
        }
        unsafe {
            let prev = self.head;
            #[cfg(feature = "valgrind-gc")]
            {
                super::valgrind::malloc_like(prev as usize, _sz);
            }
            self.head = (*prev).next;
            Address::from_ptr(prev)
        }
    }
    /// Push cell to list.
    pub fn free(&mut self, cell: Address) {
        unsafe {
            let cell = cell.to_mut_ptr::<FreeCell>();
            (*cell).next = self.head;
            #[cfg(feature = "valgrind-gc")]
            {
                super::valgrind::freelike(cell as usize);
            }
            self.head = cell;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SweepResult {
    Recyclable,
    Full,
    Free,
}

#[repr(C)]
pub struct HeapBlock {
    link: LinkedListLink,
    cell_size: usize,
    freelist: FreeList,
    free: bool,
    heap: *mut Heap,

    storage: [*mut Header; 0],
}
pub const BLOCK_SIZE: usize = 16 * 1024;
pub const CELL_ALIGN: usize = 16;
impl HeapBlock {
    /// Deallocate block and all cells allocated in it.
    ///
    ///
    /// # Safety
    ///
    /// This function is unsafe because it makes no assumptions about `this` pointer.
    ///
    pub unsafe fn destroy(this: *mut HeapBlock) {
        (*this).for_each_cell(|x| {
            let x = x as *mut Header;
            if !(*x).is_zapped() {
                std::ptr::drop_in_place((*x).get_dyn());
            }
        });

        let layout =
            Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).expect("Block alignment is wrong");
        dealloc(this as *mut Self as *mut _, layout)
    }
    pub fn set_free(&mut self, x: bool) {
        self.free = x;
    }

    pub fn is_free(&self) -> bool {
        self.free
    }

    /// Change blocks cell size.
    ///
    /// # Safety
    ///
    /// If this block is allocated then behaviour of `allocate` and `sweep` is considered undefined.
    pub unsafe fn change_cell_size(&mut self, size: usize) {
        self.cell_size = size;
    }
    pub fn create_with_cell_size(heap: *mut Heap, cell_size: usize) -> NonNull<Self> {
        unsafe {
            let layout =
                Layout::from_size_align(BLOCK_SIZE, BLOCK_SIZE).expect("Block alignment is wrong");
            let memory = alloc_zeroed(layout).cast::<Self>();
            memory.write(Self {
                heap,
                link: LinkedListLink::new(),
                cell_size,
                freelist: FreeList::new(),
                free: true,
                storage: [],
            });

            let mut freelist = FreeList::new();
            (*memory).for_each_cell(|cell| {
                //(*cell).zap(1);
                // (*cell).set_tag(GC_DEAD);

                freelist.free(Address::from_ptr(cell));
            });
            (*memory).freelist = freelist;
            NonNull::new_unchecked(memory)
        }
    }
    pub fn heap(&self) -> *mut Heap {
        self.heap
    }
    /// Walks all cells inside this block and constructs new freelist from free cells.
    ///
    ///
    /// # Safety
    ///
    /// This function is unsafe to call since `block` might point to wrong memory location or
    /// cells allocated inside block might be not properly allocated which can cause UB or segfault.
    ///
    pub unsafe fn sweep(block: *mut HeapBlock) -> SweepResult {
        let mut free = true;
        let mut freelist = FreeList::new();
        (*block).for_each_cell(|cell| unsafe {
            let cell = cell as *mut Header;
            if !(*cell).is_zapped() {
                // cell was not visited during GC, this means it is not alive and we can free it.
                if (*cell).tag() == GC_UNMARKED {
                    core::ptr::drop_in_place((*cell).get_dyn());
                    (*cell).zap(1); // mark cell as freed
                                    //(*cell).set_tag(GC_DEAD);
                    #[cfg(feature = "valgrind-gc")]
                    {
                        println!("Sweep {:p}", cell);
                    }
                    freelist.free(Address::from_ptr(cell));
                } else {
                    // cell was marked during GC, just unmark it.
                    //debug_assert_ne!((*cell).tag(), GC_GRAY);
                    (*cell).set_tag(GC_UNMARKED);
                    free = false;
                }
            } else {
                freelist.free(Address::from_ptr(cell));
            }
        });
        (*block).freelist = freelist;
        if (*block).freelist.is_empty() {
            SweepResult::Full
        } else if free {
            SweepResult::Free
        } else {
            SweepResult::Recyclable
        }
    }

    pub fn storage(&mut self) -> *mut u8 {
        self.storage.as_ptr() as usize as *mut u8
        //unsafe { round_up_to_multiple_of(16, self.storage.as_ptr() as usize) as *mut u8 }
    }

    pub fn cell(&mut self, index: usize) -> *mut Header {
        unsafe {
            let sz = self.cell_size;
            self.storage().cast::<u8>().add(index * sz).cast()
        }
    }

    pub fn cell_from_possible_pointer(&mut self, ptr: Address) -> *mut Header {
        if ptr.to_ptr::<u8>() < self.storage() as *mut _ {
            return null_mut();
        }
        let cell_index = (ptr.to_usize() - self.storage.as_ptr() as usize) / self.cell_size;
        if cell_index >= self.cell_count() {
            return null_mut();
        }
        let p = self.cell(cell_index);
        if p as usize % 16 == 0 {
            return null_mut();
        }
        p
    }
    pub fn for_each_cell(&mut self, mut cb: impl FnMut(*const Header)) {
        for i in 0..self.cell_count() {
            cb(self.cell(i));
        }
    }
    #[inline(always)]
    pub fn allocate(&mut self) -> *mut Header {
        if self.freelist.is_empty() {
            return null_mut();
        }
        let p = self.freelist.allocate(self.cell_size as _).to_mut_ptr();

        p
    }
    pub fn from_cell(addr: *mut Header) -> *mut Self {
        (addr as usize & !(BLOCK_SIZE - 1)) as *mut _
    }

    pub fn is_full(&self) -> bool {
        !self.freelist.is_empty()
    }
    pub fn cell_size(&self) -> usize {
        self.cell_size
    }
    pub fn cell_count(&self) -> usize {
        (BLOCK_SIZE - size_of::<Self>()) / self.cell_size
    }
}
