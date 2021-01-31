use super::{
    block::*, block_allocator::*, constants::*, header::Header, space_bitmap::*, util::address::*,
    util::*,
};
use core::mem::size_of;
use core::ptr::null_mut;

/// A type alias for the block, the current low and high offset.
pub type BlockTuple = (*mut ImmixBlock, u16, u16);

/// Trait for the allocators in the immix space.
///
/// Only use `get_all_blocks()` and `allocate()` from outside.
pub trait Allocator {
    /// Get all block managed by the allocator, draining any local
    /// collections.
    fn get_all_blocks(&mut self) -> std::vec::Vec<*mut ImmixBlock>;

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
    /// This allocation will be aligned (see `GCObject.object_size()`). This
    /// object is not initialized, just the memory chunk is allocated.
    ///
    /// This will try to find a hole in the `take_current_block()`. If there
    /// Is no hole `handle_no_hole()` will be called. If this function returns
    /// `None` a 'get_new_block()' is requested.
    fn allocate(&mut self, size: usize, needs_destruction: bool) -> Address {
        //!("Request to allocate an object of size {}", size);
        self.take_current_block()
            .and_then(|tp| self.scan_for_hole(size, tp))
            .or_else(|| self.handle_no_hole(size))
            .or_else(|| self.get_new_block())
            .map(|tp| self.allocate_from_block(size, tp))
            .map(|(tp, object)| {
                self.put_current_block(tp);
                unsafe { (*(tp.0)).needs_destruction += if needs_destruction { 1 } else { 0 } }
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
use std::vec::Vec;
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
    #[cfg(feature = "threaded")]
    recyc_lock: ReentrantMutex,
    #[cfg(feature = "threaded")]
    unavail_lock: ReentrantMutex,
    /// The current block to allocate from.
    current_block: Option<BlockTuple>,
}

#[cfg(feature = "threaded")]
use locks::mutex::ReentrantMutex;

impl NormalAllocator {
    /// Create a new `NormalAllocator` backed by the given `BlockAllocator`.
    pub fn new(block_allocator: *mut BlockAllocator) -> NormalAllocator {
        NormalAllocator {
            block_allocator,
            unavailable_blocks: Vec::new(),
            recyclable_blocks: Vec::new(),
            current_block: None,
            #[cfg(feature = "threaded")]
            recyc_lock: ReentrantMutex::new(),
            #[cfg(feature = "threaded")]
            unavail_lock: ReentrantMutex::new(),
        }
    }
    /// Set the recyclable blocks.
    pub fn set_recyclable_blocks(&mut self, blocks: Vec<*mut ImmixBlock>) {
        self.recyclable_blocks = blocks;
    }
}

impl Allocator for NormalAllocator {
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut blocks = Vec::new();
        for block in self
            .unavailable_blocks
            .drain(..)
            .chain(self.recyclable_blocks.drain(..))
            .chain(self.current_block.take().map(|b| b.0))
        {
            blocks.push(block);
        }
        blocks
    }

    fn take_current_block(&mut self) -> Option<BlockTuple> {
        #[cfg(not(feature = "threaded"))]
        {
            self.current_block.take()
        }
        #[cfg(feature = "threaded")]
        {
            immix_get_tls_state().current_block.take()
        }
    }

    fn put_current_block(&mut self, block_tuple: BlockTuple) {
        #[cfg(not(feature = "threaded"))]
        {
            self.current_block = Some(block_tuple);
        }
        #[cfg(feature = "threaded")]
        {
            immix_get_tls_state().current_block = Some(block_tuple);
        }
    }

    fn get_new_block(&mut self) -> Option<BlockTuple> {
        unsafe {
            let block = (&mut *self.block_allocator).get_block()?;
            (*block).allocated = true;
            Some((block, (LINE_SIZE) as u16, (BLOCK_SIZE - 1) as u16))
        }
    }

    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple> {
        if size >= LINE_SIZE {
            None
        } else {
            #[cfg(feature = "threaded")]
            {
                self.recyc_lock.lock();
            }
            match self.recyclable_blocks.pop() {
                None => {
                    #[cfg(feature = "threaded")]
                    {
                        self.recyc_lock.unlock();
                    }
                    None
                }
                Some(block) => {
                    #[cfg(feature = "threaded")]
                    {
                        self.recyc_lock.unlock();
                    }
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
        #[cfg(feature = "threaded")]
        {
            self.unavail_lock.lock();
        }
        self.unavailable_blocks.push(block);
        #[cfg(feature = "threaded")]
        {
            self.unavail_lock.unlock();
        }
    }
}

/// The `OverflowAllocator` is used to allocate *medium* sized objects
/// (objects of at least `MEDIUM_OBJECT` bytes size) within the immix space to
/// limit fragmentation in the `NormalAllocator`.
pub struct OverflowAllocator {
    /// The global `BlockAllocator` to get new blocks from.
    block_allocator: *mut BlockAllocator,
    #[cfg(feature = "threaded")]
    unavail_lock: ReentrantMutex,
    /// The exhausted blocks.
    unavailable_blocks: Vec<*mut ImmixBlock>,

    /// The current block to allocate from.
    current_block: Option<BlockTuple>,
}

impl OverflowAllocator {
    /// Create a new `OverflowAllocator` backed by the given `BlockAllocator`.
    pub fn new(block_allocator: *mut BlockAllocator) -> OverflowAllocator {
        OverflowAllocator {
            #[cfg(feature = "threaded")]
            unavail_lock: ReentrantMutex::new(),
            block_allocator,
            unavailable_blocks: Vec::new(),
            current_block: None,
        }
    }
}

impl Allocator for OverflowAllocator {
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut blocks = Vec::new();
        for block in self
            .unavailable_blocks
            .drain(..)
            .chain(self.current_block.take().map(|b| b.0))
        {
            blocks.push(block);
        }

        blocks
    }

    fn take_current_block(&mut self) -> Option<BlockTuple> {
        #[cfg(not(feature = "threaded"))]
        {
            self.current_block.take()
        }
        #[cfg(feature = "threaded")]
        {
            immix_get_tls_state().current_ovf_block.take()
        }
    }

    fn put_current_block(&mut self, block_tuple: BlockTuple) {
        #[cfg(not(feature = "threaded"))]
        {
            self.current_block = Some(block_tuple);
        }
        #[cfg(feature = "threaded")]
        {
            immix_get_tls_state().current_ovf_block = Some(block_tuple);
        }
    }

    fn get_new_block(&mut self) -> Option<BlockTuple> {
        unsafe {
            let block = (&mut *self.block_allocator).get_block()?;
            (*block).allocated = true;
            Some((block, LINE_SIZE as u16, (BLOCK_SIZE - 1) as u16))
        }
    }

    #[allow(unused_variables)]
    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple> {
        None
    }

    fn handle_full_block(&mut self, block: *mut ImmixBlock) {
        #[cfg(feature = "threaded")]
        {
            self.unavail_lock.lock();
        }
        self.unavailable_blocks.push(block);
        #[cfg(feature = "threaded")]
        {
            self.unavail_lock.unlock();
        }
    }
}
/// The `EvacAllocator` is used during the opportunistic evacuation in the
/// immix space.
///
/// It allocates from a list of up to `EVAC_HEADROOM` buffered free blocks.
///
/// _TODO_: We should not use a constant here, but something that changes
/// dynamically (see rcimmix: MAX heuristic).
pub struct EvacAllocator {
    /// The exhausted blocks.
    unavailable_blocks: Vec<*mut ImmixBlock>,

    /// The free blocks to return on 'get_new_block()'.
    evac_headroom: Vec<*mut ImmixBlock>,

    /// The current block to allocate from.
    current_block: Option<BlockTuple>,
}

impl EvacAllocator {
    /// Create a new `EvacAllocator`.
    pub fn new() -> EvacAllocator {
        EvacAllocator {
            unavailable_blocks: Vec::new(),
            evac_headroom: Vec::new(),
            current_block: None,
        }
    }

    /// Extend the list of free blocks for evacuation.
    pub fn extend_evac_headroom(&mut self, blocks: impl IntoIterator<Item = *mut ImmixBlock>) {
        self.evac_headroom.extend(blocks);
    }

    /// Get the number of currently free blocks.
    pub fn evac_headroom(&self) -> usize {
        self.evac_headroom.len()
    }
}

impl Allocator for EvacAllocator {
    fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut blocks = Vec::new();
        for block in self
            .unavailable_blocks
            .drain(..)
            .chain(self.current_block.take().map(|b| b.0))
        {
            blocks.push(block);
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
        self.evac_headroom
            .pop()
            .map(|b| unsafe {
                (*b).allocated = true;
                b
            })
            .map(|block| (block, LINE_SIZE as u16, (BLOCK_SIZE - 1) as u16))
    }

    #[allow(unused_variables)]
    fn handle_no_hole(&mut self, size: usize) -> Option<BlockTuple> {
        None
    }

    fn handle_full_block(&mut self, block: *mut ImmixBlock) {
        self.unavailable_blocks.push(block);
    }
}

/// A type which implements allocation in Immix blocks.
pub struct ImmixSpace {
    /// The global `BlockAllocator` to get new blocks from.
    pub block_allocator: *mut BlockAllocator,
    pub bitmap: SpaceBitmap,
    /// The nomal allocator for objects smaller than `MEDIUM_OBJECT` bytes.
    allocator: NormalAllocator,

    /// The overflow allocator for objects larger than `MEDIUM_OBJECT` bytes.
    overflow_allocator: OverflowAllocator,
    /// The evacuation allocator used during an evacuating collection.
    evac_allocator: EvacAllocator,
    /// The current live mark for new objects. See `Spaces.current_live_mark`.
    current_live_mark: bool,
}
impl ImmixSpace {
    pub fn filter_fast(&self, addr: Address) -> bool {
        if unsafe { !(*self.block_allocator).is_in_space(addr) } {
            return false;
        }
        true
    }
    pub fn filter(&self, addr: Address) -> Option<Address> {
        let addr = addr;
        if addr.to_usize() % 16 != 0 {
            return None;
        }
        if unsafe { (*self.block_allocator).is_in_space(addr) } && self.bitmap.test(addr.to_usize())
        {
            return Some(addr);
        }

        None
    }
    pub fn new(heap_size: usize) -> *mut Self {
        unsafe {
            let block = BlockAllocator::new(heap_size);
            let block = {
                let ptr = libc::malloc(size_of::<BlockAllocator>()).cast::<BlockAllocator>();
                ptr.write(block);
                ptr
            };
            let bitmap =
                SpaceBitmap::create("immix space bitmap", (*block).mmap.start(), heap_size);
            let mut this = Self {
                block_allocator: block,
                bitmap,
                evac_allocator: EvacAllocator::new(),
                allocator: NormalAllocator::new(null_mut()),
                overflow_allocator: OverflowAllocator::new(null_mut()),
                current_live_mark: false,
            };

            this.allocator.block_allocator =
                this.block_allocator as *const BlockAllocator as *mut _;
            this.overflow_allocator.block_allocator =
                this.block_allocator as *const BlockAllocator as *mut _;
            let ptr = libc::malloc(size_of::<Self>()).cast::<Self>();
            ptr.write(this);
            ptr
        }
    }
    /// Get the number of currently free blocks in the evacuation allocator.
    pub fn evac_headroom(&self) -> usize {
        self.evac_allocator.evac_headroom()
    }

    /// Return a collection of blocks to the global block allocator.
    pub fn return_blocks(&mut self, blocks: impl IntoIterator<Item = *mut ImmixBlock>) {
        unsafe {
            (*self.block_allocator).return_blocks(blocks);
        }
    }

    /// Set the current live mark to `current_live_mark`.
    pub fn set_current_live_mark(&mut self, current_live_mark: bool) {
        self.current_live_mark = current_live_mark;
    }

    /// Set the recyclable blocks for the `NormalAllocator`.
    pub fn set_recyclable_blocks(&mut self, blocks: Vec<*mut ImmixBlock>) {
        self.allocator.set_recyclable_blocks(blocks);
    }

    /// Extend the list of free blocks in the `EvacAllocator` for evacuation.
    pub fn extend_evac_headroom(&mut self, blocks: impl IntoIterator<Item = *mut ImmixBlock>) {
        self.evac_allocator.extend_evac_headroom(blocks);
    }
    /// Get all blocks managed by all allocators, draining any local
    /// collections.
    pub fn get_all_blocks(&mut self) -> Vec<*mut ImmixBlock> {
        let mut normal_blocks = self.allocator.get_all_blocks();
        let mut overflow_blocks = self.overflow_allocator.get_all_blocks();
        let mut evac_blocks = self.evac_allocator.get_all_blocks();
        return normal_blocks
            .drain(..)
            .chain(overflow_blocks.drain(..))
            .chain(evac_blocks.drain(..))
            .collect();
    }
    #[inline]
    pub fn allocate(&mut self, size: usize, needs_destruction: bool) -> *mut Header {
        let ptr = if size < MEDIUM_OBJECT {
            self.allocator.allocate(size, needs_destruction)
        } else {
            self.overflow_allocator.allocate(size, needs_destruction)
        };
        {
            if ptr.is_non_null() {
                let ptr = ptr.to_mut_ptr::<Header>();

                self.set_gc_object(Address::from_ptr(ptr));
            }
        }

        ptr.to_mut_ptr()
    }
    pub fn set_gc_object(&mut self, object: Address) {
        self.bitmap.set(object.to_usize())
    }
    pub fn unset_gc_object(&mut self, object: Address) {
        self.bitmap.clear(object.to_usize());
    }
    /// Might evacuate object if block of `addr` is candidate for evacuation and object is not pinned
    ///
    /// # Safety
    ///
    /// This might segfault program if `addr` is not pointing to immix space and wasn't allocated in block.
    ///
    pub unsafe fn maybe_evacuate(&mut self, addr: *mut Header) -> Option<Address> {
        let block_info = ImmixBlock::get_block_ptr(Address::from_ptr(addr));
        let is_pinned = (*addr).is_pinned();
        let is_candidate = (*block_info).evacuation_candidate;
        if is_pinned || !is_candidate {
            return None;
        }
        let size = (&*addr).size();
        let new_object = self.evac_allocator.allocate(
            align_usize(size, 16),
            (&*addr).type_info().needs_destruction,
        );
        if new_object.is_non_null() {
            core::ptr::copy_nonoverlapping(addr as *const u8, new_object.to_mut_ptr::<u8>(), size);

            self.set_gc_object(new_object);
            return Some(new_object);
        }
        None
    }
}
impl Drop for ImmixSpace {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.block_allocator);
            libc::free(self.block_allocator.cast());
        }
    }
}
