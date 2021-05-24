use super::{block::*, block_allocator::BlockAllocator};
use crate::gc::*;
use crate::gc::{accounting::space_bitmap::SpaceBitmap, mem::align_usize};

/// A type alias for the block, the current low and high offset.
pub type BlockTuple = (*mut ImmixBlock, u16, u16);

/// Trait for the allocators in the immix space.
///
/// Only use `get_all_blocks()` and `allocate()` from outside.
pub trait Allocator {
    /// Get all block managed by the allocator, draining any local
    /// collections.
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock>;

    /// Get the current block to allocate from.
    fn take_current_block(&mut self) -> Option<BlockTuple>;

    /// Set the current block to allocate from.
    fn put_current_block(&mut self, block_tuple: BlockTuple);

    /// Get a new block from a block resource.
    fn get_new_block(&mut self) -> Option<BlockTuple>;

    /// Callback if no hole of `size` bytes was found in the current block.
    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple>;

    /// Callback if the given `block` has no holes left.
    fn handle_full_block(&mut self, block: *mut ImmixBlock);

    /// Allocate an object of `size` bytes or return `None`.
    ///
    /// This allocation will be aligned. This
    /// object is not initialized, just the memory chunk is allocated.
    ///
    /// This function will try to find a hole in the `take_current_block()`. If there
    /// Is no hole `handle_no_hole()` will be called. If this function returns
    /// `None` a 'get_new_block()' is requested.
    fn allocate(&mut self, size: usize) -> Address {
        //!("Request to allocate an object of size {}", size);
        self.take_current_block()
            .and_then(|tp| self.scan_for_hole(size, tp))
            .or_else(|| self.handle_no_hole(size))
            .or_else(|| self.get_new_block())
            .map(|tp| self.allocate_from_block(size, tp))
            .map(|(tp, object)| {
                self.put_current_block(tp);

                object
            })
            .unwrap_or_else(Address::null)
    }

    /// Scan a block tuple for a hole of `size` bytes and return a matching
    /// hole.
    ///
    /// If no hole was found `handle_full_block()` is called and None
    /// returned.
    fn scan_for_hole(&mut self, size: usize, block_tuple: BlockTuple) -> Option<BlockTuple> {
        let (block, low, high) = block_tuple;
        
        match (high - low) as usize >= size {
            true => Some(block_tuple),
            false => match unsafe { (*block).scan_block(high) } {
                None => {
                    self.handle_full_block(block);
                    None
                }
                Some((low, high)) => self.scan_for_hole(size, (block, low, high)),
            },
        }
    }

    /// Allocate an uninitialized object of `size` bytes from the block tuple.
    ///
    /// Returns the block tuple with a modified low offset and the allocated
    /// object pointer.
    ///
    /// _Note_: This must only be called if there is a hole of `size` bytes
    /// starting at low offset!
    fn allocate_from_block(&self, size: usize, block_tuple: BlockTuple) -> (BlockTuple, Address) {
        let (block, low, high) = block_tuple;
        let low = align_usize(low as _, 16) as u16;
        let object = unsafe { (*block).offset(low as usize) };

        ((block, low + size as u16, high), object)
    }
}
/// The `NormalAllocator` is the standard allocator to allocate objects within
/// the immix space.
///
/// Objects smaller than `MEDIUM_OBJECT` bytes are
pub struct NormalAllocator {
    /// The global `BlockAllocator` to get new blocks from.
    block_allocator: *mut BlockAllocator,

    /// The exhausted blocks.
    unavailable_blocks: Vec<*mut ImmixBlock>,

    /// The blocks with holes to recycle before requesting new blocks..
    recyclable_blocks: Vec<*mut ImmixBlock>,

    /// The current block to allocate from.
    current_block: Option<BlockTuple>,
}
impl Allocator for NormalAllocator {
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut blocks = Vec::new();
        /*for block in self
            .unavailable_blocks
            .drain(..)
            .chain(self.recyclable_blocks.drain(..))
            .chain(self.current_block.take().map(|b| b.0))
        {
            blocks.push(block);
        }*/
        unsafe {
            if let Some((block, _, _)) = self.current_block.take() {
                blocks.push(block);
            }

            while let Some(block) = self.unavailable_blocks.pop() {
                blocks.push(block);
            }

            while let Some(block) = self.recyclable_blocks.pop() {
                blocks.push(block);
            }
        }

        blocks
    }

    fn take_current_block(&mut self) -> Option<BlockTuple> {
        self.current_block.take()
    }

    fn put_current_block(&mut self, block_tuple: BlockTuple) {
        self.current_block = Some(block_tuple);
    }

    fn get_new_block(&mut self) -> Option<BlockTuple> {
        unsafe {
            let block = (&mut *self.block_allocator).get_block()?;
            (*block).magic = IMMIX_BLOCK_MAGIC_ALLOCATED;
            Some((block, (LINE_SIZE) as u16, (BLOCK_SIZE) as u16))
        }
    }

    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple> {
        if size >= LINE_SIZE {
            None
        } else {
            match self.recyclable_blocks.pop() {
                None => None,
                Some(block) => {
                    match unsafe { (*block).scan_block((size_of::<ImmixBlock>() - 1) as u16) } {
                        None => {
                            self.handle_full_block(block);
                            self.handle_no_hole(size)
                        }
                        Some((low, high)) => {
                            debug_assert!(low as usize >= size_of::<ImmixBlock>());

                            self.scan_for_hole(size, (block, low, high))
                                .or_else(|| self.handle_no_hole(size))
                        }
                    }
                }
            }
        }
    }

    fn handle_full_block(&mut self, block: *mut ImmixBlock) {
        unsafe {
            self.unavailable_blocks.push(block);
        }
    }
}

/// The `OverflowAllocator` is used to allocate *medium* sized objects
/// (objects of at least `MEDIUM_OBJECT` bytes size) within the immix space to
/// limit fragmentation in the `NormalAllocator`.
pub struct OverflowAllocator {
    /// The global `BlockAllocator` to get new blocks from.
    block_allocator: *mut BlockAllocator,
    /// The exhausted blocks.
    unavailable_blocks: Vec<*mut ImmixBlock>,

    /// The current block to allocate from.
    current_block: Option<BlockTuple>,
}

impl Allocator for OverflowAllocator {
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut blocks = Vec::new();
        /*for block in self.unavailable_blocks.drain(..).chain(
            self.current_block
                .take()
                .map(|b| unsafe { UnsafeRef::from_raw(b.0) }),
        ) {
            blocks.push(block);
        }*/
        while let Some(block) = self.unavailable_blocks.pop() {
            blocks.push(block);
        }

        if let Some(block) = self.current_block.take() {
            blocks.push(block.0);
        }

        blocks
    }

    fn take_current_block(&mut self) -> Option<BlockTuple> {
        self.current_block.take()
    }

    fn put_current_block(&mut self, block_tuple: BlockTuple) {
        self.current_block = Some(block_tuple);
    }

    fn get_new_block(&mut self) -> Option<BlockTuple> {
        unsafe {
            let block = (&mut *self.block_allocator).get_block()?;
            (*block).magic = IMMIX_BLOCK_MAGIC_ALLOCATED;
            Some((block, LINE_SIZE as u16, (BLOCK_SIZE) as u16))
        }
    }

    #[allow(unused_variables)]
    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple> {
        None
    }

    fn handle_full_block(&mut self, block: *mut ImmixBlock) {
        unsafe {
            self.unavailable_blocks.push(block);
        }
    }
}
impl NormalAllocator {
    /// Create a new `NormalAllocator` backed by the given `BlockAllocator`.
    pub fn new(block_allocator: *mut BlockAllocator) -> NormalAllocator {
        NormalAllocator {
            block_allocator,
            unavailable_blocks: vec![],
            recyclable_blocks: vec![],
            current_block: None,
        }
    }
    /// Set the recyclable blocks.
    pub fn set_recyclable_blocks(&mut self, blocks: Vec<*mut ImmixBlock>) {
        self.recyclable_blocks = blocks;
    }
}

impl OverflowAllocator {
    /// Create a new `OverflowAllocator` backed by the given `BlockAllocator`.
    pub fn new(block_allocator: *mut BlockAllocator) -> OverflowAllocator {
        OverflowAllocator {
            block_allocator,
            unavailable_blocks: vec![],
            current_block: None,
        }
    }
}

pub struct ImmixSpace {
    pub block_allocator: *mut BlockAllocator,
    pub bitmap: SpaceBitmap<16>,
    allocator: NormalAllocator,
    overflow_allocator: OverflowAllocator,
    allocated: usize,
    pub unavail: Vec<*mut ImmixBlock>,
}

impl ImmixSpace {
    pub fn allocated(&self) -> usize {
        self.allocated
    }
    pub fn filter_fast(&self, addr: Address) -> bool {
        unsafe { (*self.block_allocator).is_in_space(addr) }
    }

    pub fn filter(&self, addr: Address) -> bool {
        if addr.to_usize() % 16 != 0 {
            return false;
        }
        self.filter_fast(addr) && self.bitmap.test(addr.to_usize())
    }
    pub fn new(size: usize) -> Self {
        unsafe {
            let block_allocator = Box::into_raw(Box::new(BlockAllocator::new(size)));
            let bitmap = SpaceBitmap::<16>::create("immix", (*block_allocator).mmap.start(), size);
            let mut this = Self {
                block_allocator,
                allocated: 0,
                unavail: vec![],
                bitmap,
                allocator: NormalAllocator::new(block_allocator),
                overflow_allocator: OverflowAllocator::new(block_allocator),
            };

            this
        }
    }

    /// Return a collection of blocks to the global block allocator.
    pub fn return_blocks(&mut self, blocks: impl IntoIterator<Item = *mut ImmixBlock>) {
        unsafe {
            (*self.block_allocator).return_blocks(blocks);
        }
    }

    /// Set the recyclable blocks for the `NormalAllocator`.
    pub fn set_recyclable_blocks(&mut self, blocks: Vec<*mut ImmixBlock>) {
        self.allocator.set_recyclable_blocks(blocks);
    }

    /// Get all blocks managed by all allocators, draining any local
    /// collections.
    pub fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut normal_blocks = self.allocator.get_all_blocks();
        let mut overflow_blocks = self.overflow_allocator.get_all_blocks();

        let mut all_blocks = Vec::new();
        while let Some(block) = normal_blocks.pop() {
            all_blocks.push(block);
        }

        while let Some(block) = overflow_blocks.pop() {
            all_blocks.push(block);
        }
        while let Some(block) = self.unavail.pop() {
            all_blocks.push(block);
        }
        all_blocks
    }
    #[inline]
    pub fn allocate(&mut self, size: usize) -> *mut GcPointerBase {
        let ptr = if size < LINE_SIZE {
            self.allocator.allocate(size)
        } else {
            self.overflow_allocator.allocate(size)
        };
        {
            if ptr.is_non_null() {
                self.allocated += size;
                let ptr = ptr.to_mut_ptr::<GcPointerBase>();

                self.set_gc_object(Address::from_ptr(ptr));
            }
        }

        ptr.to_mut_ptr()
    }
    pub fn set_gc_object(&mut self, object: Address) -> bool {
        self.bitmap.set(object.to_usize())
    }
    pub fn unset_gc_object(&mut self, object: Address) {
        self.bitmap.clear(object.to_usize());
    }

    pub fn walk(&mut self, cb: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        self.bitmap.walk(|ptr| {
            cb(ptr as _, 16);
        });
    }

    pub unsafe fn sweep(&mut self) -> usize {
        let mut scan = (*self.block_allocator).data;
        let end = (*self.block_allocator).data_bound;
        while scan < end {
            if self.bitmap.test(scan as _) {
                let object = scan.cast::<GcPointerBase>();
                if (*object).state() == DEFINETELY_WHITE {
                    let addr = Address::from_ptr(object);
                    (*ImmixBlock::get_block_ptr(addr)).line_object_unmark(addr);
                    self.allocated -= (*object).allocation_size();
                    core::ptr::drop_in_place((*object).get_dyn());
                    self.bitmap.clear(scan as _);
                }
            }
            scan = scan.add(16);
        }
        self.allocated
    }
}

impl Drop for ImmixSpace {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.block_allocator);
        }
    }
}
