use crate::runtime::{ref_ptr::Ref, type_info::TypeInfo, vm::JsVirtualMachine};

use super::header::Header;
use super::util::address::Address;
use core::sync::atomic::AtomicBool;
/// Precise allocation used for large objects (>= LARGE_CUTOFF).
/// Wafflelink uses GlobalAlloc that already knows what to do for large allocations. The GC shouldn't
/// have to think about such things. That's where PreciseAllocation comes in. We will allocate large
/// objects directly using malloc, and put the PreciseAllocation header just before them. We can detect
/// when a *mut Object is a PreciseAllocation because it will have the ATOM_SIZE / 2 bit set.
#[repr(C)]
pub struct PreciseAllocation {
    //pub link: LinkedListLink,
    /// allocation request size
    pub cell_size: usize,
    /// Is this allocation marked by GC?
    pub is_marked: bool,
    /// index in precise_allocations
    pub index_in_space: u32,
    /// Is alignment adjusted?
    //pub is_newly_allocated: bool,
    pub adjusted_alignment: bool,
    /// Is this even valid allocation?
    pub has_valid_cell: bool,
    pub vm: Ref<JsVirtualMachine>,
}

impl PreciseAllocation {
    /// Return atomic reference to `is_marked`
    pub fn mark_atomic(&self) -> &AtomicBool {
        unsafe { &*(&self.is_marked as *const bool as *const AtomicBool) }
    }
    /// Alignment of allocation.
    pub const ALIGNMENT: usize = 16;
    /// Alignment of pointer returned by `Self::cell`.
    pub const HALF_ALIGNMENT: usize = Self::ALIGNMENT / 2;
    /// Check if raw_ptr is precisely allocated.
    pub fn is_precise(raw_ptr: *mut ()) -> bool {
        (raw_ptr as usize & Self::HALF_ALIGNMENT) != 0
    }
    /// Create PreciseAllocation from pointer
    pub fn from_cell(ptr: *mut Header) -> *mut Self {
        unsafe {
            ptr.cast::<u8>()
                .offset(-(Self::header_size() as isize))
                .cast()
        }
    }
    /// Return base pointer
    #[inline]
    pub fn base_pointer(&self) -> *mut () {
        if self.adjusted_alignment {
            ((self as *const Self as isize) - (Self::HALF_ALIGNMENT as isize)) as *mut ()
        } else {
            self as *const Self as *mut ()
        }
    }
    /// Set `is_marked` to false
    pub fn clear_marked(&mut self) {
        self.is_marked = false;
        //self.is_marked.store(false, Ordering::Relaxed);
    }
    /// Return `is_marked`
    pub fn is_marked(&self) -> bool {
        self.is_marked
    }
    /// Test and set marked. Will return true
    /// if it is already marked.
    pub fn test_and_set_marked(&mut self) -> bool {
        if self.is_marked() {
            return true;
        }
        self.is_marked = true;
        false
    }

    /// Return cell address, it is always aligned to `Self::HALF_ALIGNMENT`.
    pub fn cell(&self) -> *mut Header {
        let addr = Address::from_ptr(self).offset(Self::header_size());
        addr.to_mut_ptr()
    }
    /// Return true if raw_ptr is above lower bound
    pub fn above_lower_bound(&self, raw_ptr: *mut ()) -> bool {
        let ptr = raw_ptr;
        let begin = self.cell() as *mut ();
        ptr >= begin
    }
    /// Return true if raw_ptr below upper bound
    pub fn below_upper_bound(&self, raw_ptr: *mut ()) -> bool {
        let ptr = raw_ptr;
        let begin = self.cell() as *mut ();
        let end = (begin as usize + self.cell_size) as *mut ();
        ptr <= (end as usize + 8) as *mut ()
    }
    /// Returns header size + required alignment to make cell be aligned to 8.
    pub const fn header_size() -> usize {
        ((core::mem::size_of::<PreciseAllocation>() + Self::HALF_ALIGNMENT - 1)
            & !(Self::HALF_ALIGNMENT - 1))
            | Self::HALF_ALIGNMENT
    }
    /// Does this allocation contains raw_ptr?
    pub fn contains(&self, raw_ptr: *mut ()) -> bool {
        self.above_lower_bound(raw_ptr) && self.below_upper_bound(raw_ptr)
    }
    /// Is this allocation live?
    pub fn is_live(&self) -> bool {
        self.is_marked() //|| self.is_newly_allocated
    }
    /// Clear mark bit
    pub fn flip(&mut self) {
        self.clear_marked();
    }
    /// Is this marked?
    pub fn is_empty(&self) -> bool {
        !self.is_marked() //&& !self.is_newly_allocated
    }
    /// Derop cell if this allocation is not marked.
    pub fn sweep(&mut self) -> bool {
        if self.has_valid_cell && !self.is_live() {
            self.has_valid_cell = false;
            let cell = self.cell();
            unsafe {
                if let Some(fin) = (*cell).type_info().destructor {
                    fin(Address::from_ptr(cell));
                }
            }
            return false;
        }
        true
    }
    /// Try to create precise allocation (no way that it will return null for now).
    pub fn try_create(vm: Ref<JsVirtualMachine>, size: usize, index_in_space: u32) -> *mut Self {
        let adjusted_alignment_allocation_size = Self::header_size() + size + Self::HALF_ALIGNMENT;
        unsafe {
            let mut space = libc::malloc(adjusted_alignment_allocation_size).cast::<u8>();
            //let mut space = libc::malloc(adjusted_alignment_allocation_size);
            let mut adjusted_alignment = false;
            if !is_aligned_for_precise_allocation(space) {
                space = space.add(Self::HALF_ALIGNMENT);
                adjusted_alignment = true;
                assert!(is_aligned_for_precise_allocation(space));
            }
            assert!(size != 0);
            space.cast::<Self>().write(Self {
                vm,
                //link: LinkedListLink::new(),
                adjusted_alignment,
                is_marked: false,
                //is_newly_allocated: true,
                has_valid_cell: true,
                cell_size: size,
                index_in_space,
            });

            space.cast()
        }
    }
    /// return cell size
    pub fn cell_size(&self) -> usize {
        self.cell_size
    }
    /// Destroy this allocation
    pub fn destroy(&mut self) {
        let base = self.base_pointer();
        let cell = self.cell();
        unsafe {
            if self.has_valid_cell {
                self.has_valid_cell = false;
                if let Some(fin) = (*cell).type_info().destructor {
                    fin(Address::from_ptr(cell));
                }
            }
        }
        unsafe {
            libc::free(base.cast());
        }
    }
}
/// Check if `mem` is aligned for precise allocation
pub fn is_aligned_for_precise_allocation(mem: *mut u8) -> bool {
    let allocable_ptr = mem as usize;
    (allocable_ptr & (PreciseAllocation::ALIGNMENT - 1)) == 0
}

/// This space contains objects which are larger than the size limits of other spaces.
/// Each object gets its own malloc'd region of memory.
/// Large objects are never moved by the garbage collector.
pub struct LargeObjectSpace {
    pub(crate) allocations: std::vec::Vec<*mut PreciseAllocation>,
    pub(crate) current_live_mark: bool,
    pub(crate) vm: Ref<JsVirtualMachine>,
}

impl LargeObjectSpace {
    pub fn new(vm: Ref<JsVirtualMachine>) -> Self {
        Self {
            vm,
            current_live_mark: false,
            allocations: std::vec::Vec::with_capacity(8),
        }
    }
    pub fn sweep(&mut self) -> usize {
        let mut sweeped = 0;
        self.allocations.retain(|ptr| unsafe {
            let p = &mut **ptr;
            let retain = p.sweep();
            if !retain {
                p.destroy();
                sweeped += p.cell_size;
            }
            retain
        });
        self.allocations.sort_unstable();
        sweeped
    }
    #[allow(clippy::collapsible_if)]
    pub fn contains(&self, p: Address) -> bool {
        if self.allocations.is_empty() {
            return false;
        }
        unsafe {
            if (&*self.allocations[0]).above_lower_bound(p.to_mut_ptr())
                && (&**self.allocations.last().unwrap()).below_upper_bound(p.to_mut_ptr())
            {
                if self
                    .allocations
                    .binary_search(&PreciseAllocation::from_cell(p.to_mut_ptr()))
                    .is_ok()
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn alloc(&mut self, size: usize) -> Address {
        let ix = self.allocations.len() as u32;
        let cell = PreciseAllocation::try_create(self.vm, size, ix);
        unsafe {
            if cell.is_null() {
                return Address::null();
            }
            self.allocations.push(cell);
            if (cell as usize) < self.allocations[self.allocations.len() - 1] as usize
                || (cell as usize) < self.allocations[0] as usize
            {
                self.allocations.sort_unstable();
            }

            Address::from_ptr(cell)
        }
    }
}

impl Drop for LargeObjectSpace {
    fn drop(&mut self) {
        while let Some(alloc) = self.allocations.pop() {
            unsafe {
                (&mut *alloc).destroy();
            }
        }
    }
}
