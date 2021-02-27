use std::{
    alloc::{alloc, Layout},
    ptr::null_mut,
};
/// Block size must be at least as large as the system page size.
pub const BLOCK_SIZE: usize = 16 * 1024;
/// Single atom size
pub const ATOM_SIZE: usize = 16;
/// Numbers of atoms per block
pub const ATOMS_PER_BLOCK: usize = BLOCK_SIZE / ATOM_SIZE;
/// Lower tiers maximum
pub const MAX_NUMBER_OF_LOWER_TIER_CELLS: usize = 8;
/// End atom offset
pub const END_ATOM: usize = (BLOCK_SIZE - core::mem::size_of::<BlockHeader>()) / ATOM_SIZE;
/// Block payload size
pub const PAYLOAD_SIZE: usize = END_ATOM * ATOM_SIZE;
/// Block header size
pub const FOOTER_SIZE: usize = BLOCK_SIZE - PAYLOAD_SIZE;
/// Atom alignment mask
pub const ATOM_ALIGNMENT_MASK: usize = ATOM_SIZE - 1;

use intrusive_collections::LinkedListLink;

use super::{cell::GcPointerBase, Heap, SweepResult};

pub const BLOCK_LO_MASK: usize = BLOCK_SIZE - 1;
pub const BLOCK_HI_MASK: usize = !BLOCK_LO_MASK;

/// Single freelist cell.
#[repr(C)]
pub struct FreeCell {
    bytes: u64,
    next: *mut Self,
}

/// Singly linked list used as free-list
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
    pub fn allocate(&mut self) -> *mut u8 {
        if self.is_empty() {
            return null_mut();
        }
        unsafe {
            let prev = self.head;
            self.head = (&*prev).next;
            prev.cast()
        }
    }
    /// Push cell to list.
    pub fn free(&mut self, cell: *mut u8) {
        unsafe {
            let cell = cell.cast::<FreeCell>();
            (&mut *cell).next = self.head;
            self.head = cell;
        }
    }
}
use intrusive_collections::intrusive_adapter;
use intrusive_collections::UnsafeRef;
intrusive_adapter!(pub BlockAdapter = UnsafeRef<BlockHeader> : BlockHeader {link: LinkedListLink});

/// Atom representation
pub type Atom = [u8; ATOM_SIZE];
/// Heap allocated block header
pub struct BlockHeader {
    pub link: LinkedListLink,
    pub heap: *mut Heap,
    pub cell_size: u32,
    /// Free list for allocation
    pub freelist: FreeList,
    /// Pointer to block.
    pub block: *mut Block,
}

// Block is a page-aligned container for heap-allocated objects.
/// Objects are allocated within cells of the marked block. For a given
/// marked block, all cells have the same size. Objects smaller than the
/// cell size may be allocated in the marked block, in which case the
/// allocation suffers from internal fragmentation: wasted space whose
/// size is equal to the difference between the cell size and the object
/// size.
pub struct Block {}

impl Block {
    pub fn destroy(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.header());
            std::alloc::dealloc(
                self as *mut Self as *mut u8,
                Layout::from_size_align_unchecked(BLOCK_SIZE, BLOCK_SIZE),
            );
        }
    }
    /// Get block header
    pub fn header(&self) -> &mut BlockHeader {
        unsafe { &mut *self.atoms().offset(END_ATOM as _).cast() }
    }
    /// Atom offset from pointer
    pub fn atom_number(&self, p: *mut u8) -> u32 {
        let atom_n = self.candidate_atom_number(p);
        atom_n as _
    }
    /// Atom offset from pointer, might be wrong
    pub fn candidate_atom_number(&self, p: *mut u8) -> usize {
        return (p as usize - self as *const Self as usize) / ATOM_SIZE;
    }
    /// Pointer to atoms
    pub fn atoms(&self) -> *mut Atom {
        self as *const Self as *mut Atom
    }
    /// Is pointer aligned to atom size?
    pub fn is_atom_aligned(p: *mut u8) -> bool {
        (p as usize & ATOM_ALIGNMENT_MASK) == 0
    }
    /// Try to get block from pointer
    pub fn from_cell(p: *mut u8) -> *mut Self {
        (p as usize & (!(BLOCK_SIZE - 1))) as *mut Self
    }
    /// Allocate new block and instantiate freelist.
    pub fn new(cell_size: usize, heap: *mut Heap) -> &'static mut BlockHeader {
        unsafe {
            let memory =
                alloc(Layout::from_size_align_unchecked(BLOCK_SIZE, BLOCK_SIZE)).cast::<Self>();
            ((&*memory).header() as *mut BlockHeader).write(BlockHeader {
                link: LinkedListLink::new(),
                cell_size: cell_size as _,
                freelist: FreeList::new(),
                block: memory,
                heap,
            });
            let mut count = 0;
            (&*memory).header().for_each_cell(|cell| {
                (*cell.cast::<GcPointerBase>()).dead();
                count += 1;
                (&mut *memory).header().freelist.free(cell);
            });
            (&*memory).header()
        }
    }
}

impl BlockHeader {
    /// Atoms per cell
    pub const fn atoms_per_cell(&self) -> usize {
        ((self.cell_size as usize + ATOM_SIZE - 1) / ATOM_SIZE) as _
    }
    /// Offset of end atom
    pub const fn end_atom(&self) -> usize {
        END_ATOM - self.atoms_per_cell() + 1
    }
    /// Cell size
    pub const fn cell_size(&self) -> u32 {
        self.cell_size
    }
    /// Start of the block
    pub fn begin(&self) -> *mut u8 {
        self.block.cast()
    }
    /// Iterate through each cell.
    pub fn for_each_cell(&self, mut func: impl FnMut(*mut u8)) {
        /*let mut i = self.cell_count();
        while i > 0 {
            func(self.cell(i as _));
            i -= 1;
        }*/
        let mut i = 0;
        while i < self.end_atom() - 1 {
            let cell = unsafe { self.atoms().offset(i as _) };
            func(cell.cast());
            i += self.atoms_per_cell();
        }
    }
    /// Return cell at `index`
    pub fn cell(&self, index: usize) -> *mut u8 {
        unsafe { self.begin().add(index * self.cell_size() as usize) }
    }
    /// Cell count
    pub const fn cell_count(&self) -> u32 {
        (BLOCK_SIZE as u32 - core::mem::size_of::<Self>() as u32) / self.cell_size()
    }
    /// Try to allocate memory of `cell_size` bytes.
    pub fn allocate(&mut self) -> *mut u8 {
        let addr = self.freelist.allocate();

        addr
    }

    /// Atom number
    pub fn atom_number(&self, p: *mut u8) -> u32 {
        let atom_n = self.candidate_atom_number(p);
        atom_n as _
    }
    pub fn is_atom(&self, p: *mut u8) -> bool {
        let an = self.candidate_atom_number(p);
        if an % self.atoms_per_cell() != 0 {
            return false;
        }
        if an >= self.end_atom() || an < self.begin() as usize {
            return false;
        }
        true
    }

    pub fn is_live(&self, p: *mut u8) -> bool {
        if self.is_atom(p) {
            unsafe {
                let cell = p.cast::<GcPointerBase>();
                (*cell).is_live()
            }
        } else {
            false
        }
    }
    /// Atom number
    pub fn candidate_atom_number(&self, p: *mut u8) -> usize {
        return (p as usize - self.begin() as usize) / ATOM_SIZE;
    }
    /// Atoms pointer
    pub fn atoms(&self) -> *mut Atom {
        self.begin().cast()
    }
    pub fn cell_align(&self, p: *const ()) -> *const () {
        let base = self.atoms() as usize;
        let mut bits = p as usize;
        bits -= base;
        bits -= bits % self.cell_size() as usize;
        bits += base;
        bits as *const ()
    }

    pub unsafe fn sweep(&mut self) -> SweepResult {
        let mut free = true;
        let mut freelist = FreeList::new();
        self.for_each_cell(|cell| {
            let cell = cell as *mut GcPointerBase;
            if (*cell).is_live() {
                if !(*cell).is_marked() {
                    println!("sweep {:p}", cell);
                    std::ptr::drop_in_place((*cell).get_dyn());
                    (*cell).dead();
                    freelist.free(cell.cast());
                } else {
                    (*cell).unmark();
                    free = false;
                }
            } else {
                freelist.free(cell.cast());
            }
        });
        self.freelist = freelist;
        if self.freelist.is_empty() {
            SweepResult::Full
        } else if free {
            SweepResult::Free
        } else {
            SweepResult::Recyclable
        }
    }
}
