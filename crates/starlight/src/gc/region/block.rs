use crate::{
    gc::{cell::GcPointerBase, mem::align_usize, Address},
    space_bitmap_gen,
};

use intrusive_collections::LinkedListLink;
use intrusive_collections::{intrusive_adapter, UnsafeRef};
intrusive_adapter! {
    pub BlockAdapter = UnsafeRef<ImmixBlock> : ImmixBlock {link: LinkedListLink}
}
pub const LINE_SIZE: usize = 256;
pub const BLOCK_SIZE: usize = 32 * 1024;
pub const NUM_LINES_PER_BLOCK: usize = BLOCK_SIZE / LINE_SIZE;
space_bitmap_gen!(LineMap, LINE_SIZE, BLOCK_SIZE as u64);

pub const IMMIX_BLOCK_MAGIC_ALLOCATED: u32 = 0xdeadbeef;

/// Immix Block type.
#[repr(C)]
pub struct ImmixBlock {
    pub(super) link: LinkedListLink,
    /// Bitmap to find *free* lines.
    pub(super) line_map: LineMap,
    /// How much free lines is there.
    pub(super) hole_count: u32,
    /// Magic number to check if block is allocated. Used only for conservative scanning.
    pub(super) magic: u32,
}

impl ImmixBlock {
    /// Get pointer to block from `object` pointer.
    ///
    /// # Safety
    /// Does not do anything unsafe but might return wrong pointer
    pub unsafe fn get_block_ptr(object: Address) -> *mut Self {
        let off = object.to_usize() % BLOCK_SIZE;
        (object.to_mut_ptr::<u8>()).offset(-(off as isize)) as *mut ImmixBlock
    }

    pub fn new(at: *mut u8) -> &'static mut Self {
        unsafe {
            let ptr = at as *mut Self;
            debug_assert!(ptr as usize % 32 * 1024 == 0);
            ptr.write(Self {
                link: LinkedListLink::new(),
                line_map: LineMap::new(),
                magic: 0,
                hole_count: 0,
            });

            &mut *ptr
        }
    }
    #[inline]
    pub fn is_in_block(&self, p: Address) -> bool {
        if self.magic == IMMIX_BLOCK_MAGIC_ALLOCATED {
            let b = self.begin();
            let e = b + BLOCK_SIZE;
            b < p.to_usize() && p.to_usize() <= e
        } else {
            false
        }
    }
    /*#[inline]
    pub fn is_gc_object(&self, p: Address) -> bool {
        if self.is_in_block(p) {
            self.object_map.test(p.to_usize(), self.begin())
        } else {
            false
        }
    }*/
    pub fn begin(&self) -> usize {
        self as *const Self as usize
    }
    /// Scan the block for a hole to allocate into.
    ///
    /// The scan will start at `last_high_offset` bytes into the block and
    /// return a tuple of `low_offset`, `high_offset` as the lowest and
    /// highest usable offsets for a hole.
    ///
    /// `None` is returned if no hole was found.
    pub fn scan_block(&self, last_high_offset: u16) -> Option<(u16, u16)> {
        let last_high_index = last_high_offset as usize / LINE_SIZE;
        let mut low_index = NUM_LINES_PER_BLOCK - 1;

        for index in (last_high_index + 1)..NUM_LINES_PER_BLOCK {
            if !self
                .line_map
                .test(self.begin() + (index * LINE_SIZE), self.begin())
            {
                low_index = index + 1;
                break;
            }
        }
        let mut high_index = NUM_LINES_PER_BLOCK;
        for index in low_index..NUM_LINES_PER_BLOCK {
            if self
                .line_map
                .test(self.begin() + (LINE_SIZE * index), self.begin())
            {
                high_index = index;
                break;
            }
        }

        if low_index == high_index && high_index != (NUM_LINES_PER_BLOCK - 1) {
            return self.scan_block((high_index * LINE_SIZE - 1) as u16);
        } else if low_index < (NUM_LINES_PER_BLOCK - 1) {
            return Some((
                align_usize(low_index * LINE_SIZE, 16) as u16,
                (high_index * LINE_SIZE - 1) as u16,
            ));
        }

        None
    }
    pub fn count_holes(&mut self) -> usize {
        let mut holes: usize = 0;
        let mut in_hole = false;
        let b = self.begin();
        for i in 0..NUM_LINES_PER_BLOCK {
            match (in_hole, self.line_map.test(b + (LINE_SIZE * i), b)) {
                (false, false) => {
                    holes += 1;
                    in_hole = true;
                }
                (_, _) => {
                    in_hole = false;
                }
            }
        }
        self.hole_count = holes as _;
        holes
    }
    pub fn offset(&self, offset: usize) -> Address {
        Address::from(self.begin() + offset)
    }

    pub fn is_empty(&self) -> bool {
        for i in 0..NUM_LINES_PER_BLOCK {
            if self
                .line_map
                .test(self.begin() + (i * LINE_SIZE), self.begin())
            {
                return false;
            }
        }
        true
    }
    /// Update the line mark for the given object.
    #[inline(always)]
    fn modify_line(&mut self, object: Address, mark: bool) {
        let line_num = Self::object_to_line_num(object);
        let b = self.begin();

        let object_ptr = object.to_mut_ptr::<GcPointerBase>();
        unsafe {
            let obj = &mut *object_ptr;

            let size = obj.allocation_size();

            for line in line_num..(line_num + (size / LINE_SIZE) + 1) {
                if mark {
                    self.line_map.set(b + (line * LINE_SIZE), b);
                } else {
                    self.line_map.clear(b + (line * LINE_SIZE), b);
                }
            }
        }
    }
    /// Return the number of holes and marked lines in this block.
    ///
    /// A marked line is a line with a count of at least one.
    ///
    /// _Note_: You must call count_holes() bevorhand to set the number of
    /// holes.
    pub fn count_holes_and_marked_lines(&self) -> (usize, usize) {
        (self.hole_count as usize, {
            let mut count = 0;
            for line in 0..NUM_LINES_PER_BLOCK {
                if self
                    .line_map
                    .test(line * LINE_SIZE + self.begin(), self.begin())
                {
                    count += 1;
                }
            }
            count
        })
    }

    /// Return the number of holes and available lines in this block.
    ///
    /// An available line is a line with a count of zero.
    ///
    /// _Note_: You must call count_holes() bevorhand to set the number of
    /// holes.
    pub fn count_holes_and_available_lines(&self) -> (usize, usize) {
        (self.hole_count as usize, {
            let mut count = 0;
            for line in 0..NUM_LINES_PER_BLOCK {
                if !self
                    .line_map
                    .test(line * LINE_SIZE + self.begin(), self.begin())
                {
                    count += 1;
                }
            }
            count
        })
    }
    pub fn reset(&mut self) {
        self.line_map.clear_all();
        // self.object_map.clear_all();
        self.magic = 0;
        self.hole_count = 0;
        //self.evacuation_candidate = false;
    }
    pub fn line_object_mark(&mut self, object: Address) {
        self.modify_line(object, true);
    }

    pub fn line_object_unmark(&mut self, object: Address) {
        self.modify_line(object, false);
    }
    pub fn line_is_marked(&self, line: usize) -> bool {
        self.line_map
            .test(self.begin() + (line * LINE_SIZE), self.begin())
    }

    pub fn object_to_line_num(object: Address) -> usize {
        (object.to_usize() % BLOCK_SIZE) / LINE_SIZE
    }
}
