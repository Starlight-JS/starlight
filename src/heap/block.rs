use crate::runtime::{ref_ptr::Ref, vm::JSVirtualMachine};

use super::constants::*;
use super::header::Header;
use super::util::address::Address;
use super::util::align_usize;
// LineMap is used for scanning block for holes
space_bitmap_gen!(LineMap, LINE_SIZE, BLOCK_SIZE as u64);

pub struct ImmixBlock {
    /// This bitmap stores mark state of each block line.
    ///
    ///
    /// When allocating we search for hole using this bitmap. Hole is space between unmarked line
    /// and marked.
    pub line_map: LineMap,
    /// Indicates if this block is actually allocated.
    pub allocated: bool,
    /// How many holes block had at the moment of invoking `count_holes`.
    pub hole_count: u32,
    /// Is this block a candidate for evacuation in GC cycle? Block is evacuated
    /// when there is not much holes left for allocation.
    pub evacuation_candidate: bool,
    /// Set to true if this block has any object allocated that needs destruction.
    pub needs_destruction: u32,
    pub vm: Ref<JSVirtualMachine>,
}

impl ImmixBlock {
    /// Get pointer to block from `object` pointer.
    ///
    ///
    /// NOTE: This is not guaranteed to return correct block pointer. Additional checks must be done
    /// before dereferencing pointer.
    pub fn get_block_ptr(object: Address) -> *mut Self {
        let off = object.to_usize() % BLOCK_SIZE;
        unsafe { (object.to_mut_ptr::<u8>()).offset(-(off as isize)) as *mut ImmixBlock }
    }

    /// Creates new block at `at` memory location and returns mutable reference to it.
    ///
    ///
    /// NOTE: `at` pointer *must* be aligned to 32KiB and be writable.
    ///
    pub fn new(at: *mut u8, vm: Ref<JSVirtualMachine>) -> &'static mut Self {
        unsafe {
            let ptr = at as *mut Self;
            debug_assert!(ptr as usize % 32 * 1024 == 0);
            ptr.write(Self {
                line_map: LineMap::new(),
                allocated: false,
                hole_count: 0,
                needs_destruction: 0,
                evacuation_candidate: false,
                vm,
            });

            &mut *ptr
        }
    }
    /// Checks if `p` is somewhere in the block. Returns false if block is not allocated
    /// or address is not in the block.
    #[inline]
    pub fn is_in_block(&self, p: Address) -> bool {
        if self.allocated {
            let b = self.begin();
            let e = b + BLOCK_SIZE;
            b < p.to_usize() && p.to_usize() <= e
        } else {
            false
        }
    }
    /// Return begin memory location  of the block.
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
        /*debug!(
            "Scanning block {:p} for a hole with last_high_offset {}",
            self, last_high_index
        );*/
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
            //debug!("Rescan: Found single line hole? in block {:p}", self);
            return self.scan_block((high_index * LINE_SIZE - 1) as u16);
        } else if low_index < (NUM_LINES_PER_BLOCK - 1) {
            /* debug!(
                "Found low index {} and high index {} in block {:p}",
                low_index, high_index, self
            );*/

            /*debug!(
                "Index offsets: ({},{})",
                low_index * LINE_SIZE,
                high_index * LINE_SIZE - 1
            );*/
            return Some((
                align_usize(low_index * LINE_SIZE, 16) as u16,
                (high_index * LINE_SIZE - 1) as u16,
            ));
        }
        //debug!("Found no hole in block {:p}", self);

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
    /// Update the line counter for the given object.
    ///
    /// Increment if `increment`, otherwise do a saturating substraction.
    #[inline(always)]
    fn modify_line(&mut self, object: Address, mark: bool) {
        let line_num = Self::object_to_line_num(object);
        let b = self.begin();

        let object_ptr = object.to_mut_ptr::<Header>();
        unsafe {
            let obj = &mut *object_ptr;

            let size = obj.size();

            for line in line_num..(line_num + (size / LINE_SIZE) + 1) {
                if mark {
                    self.line_map.set(b + (line * LINE_SIZE), b);
                //debug_assert!(self.line_map.test(b + (line * LINE_SIZE), b));
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
        self.allocated = false;
        self.hole_count = 0;
        self.evacuation_candidate = false;
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
