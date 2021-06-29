use core::fmt;
use core::mem::size_of;
use memmap2::MmapMut;
use wtf_rs::round_up;
pub struct SpaceBitmap<const ALIGNMENT: usize> {
    mem_map: MmapMut,
    bitmap_begin: *mut usize,
    bitmap_size: usize,
    heap_begin: usize,
    heap_limit: usize,
}

impl<const ALIGNMENT: usize> SpaceBitmap<ALIGNMENT> {
    pub const fn offset_bit_index(offset: usize) -> usize {
        (offset / ALIGNMENT) % (size_of::<usize>() * 8)
    }

    pub const fn offset_to_index(offset: usize) -> usize {
        offset / ALIGNMENT / (size_of::<usize>() * 8)
    }

    pub const fn index_to_offset(index: usize) -> isize {
        index as isize * ALIGNMENT as isize * (size_of::<usize>() as isize * 8)
    }

    pub const fn offset_to_mask(offset: usize) -> usize {
        1 << Self::offset_bit_index(offset)
    }

    pub fn compute_bitmap_size(capacity: u64) -> usize {
        let bytes_covered_per_word = ALIGNMENT * (size_of::<usize>() * 8);
        (round_up(capacity as usize, bytes_covered_per_word as _) / bytes_covered_per_word as usize)
            as usize
            * size_of::<isize>()
    }

    pub fn compute_heap_size(bitmap_bytes: u64) -> usize {
        (bitmap_bytes * 8 * ALIGNMENT as u64) as _
    }

    pub fn new(
        name: &str,
        mem_map: MmapMut,
        bitmap_begin: *mut usize,
        bitmap_size: usize,
        heap_begin: *mut u8,
        heap_capacity: usize,
    ) -> Self {
        Self {
            mem_map,
            bitmap_size,
            bitmap_begin: bitmap_begin.cast::<usize>(),

            heap_begin: heap_begin as _,
            heap_limit: heap_begin as usize + heap_capacity,
        }
    }

    pub fn create_from_memmap(
        name: &str,
        mem_map: MmapMut,
        heap_begin: *mut u8,
        heap_capacity: usize,
    ) -> Self {
        let bitmap_begin = mem_map.as_ptr() as *mut u8;
        let bitmap_size = Self::compute_bitmap_size(heap_capacity as _);
        Self {
            mem_map,
            bitmap_begin: bitmap_begin.cast(),
            bitmap_size,
            heap_begin: heap_begin as _,
            heap_limit: heap_begin as usize + heap_capacity,
        }
    }

    pub fn create(name: &str, heap_begin: *mut u8, heap_capacity: usize) -> Self {
        let bitmap_size = Self::compute_bitmap_size(heap_capacity as _);

        let mem_map = MmapMut::map_anon(bitmap_size).unwrap();
        Self::create_from_memmap(name, mem_map, heap_begin, heap_capacity)
    }

    #[inline]
    pub fn test(&self, object: usize) -> bool {
        let offset = object as isize - self.heap_begin as isize;
        let index = Self::offset_to_index(offset as _);
        let mask = Self::offset_to_mask(offset as _);
        let atomic_entry = unsafe { *self.bitmap_begin.add(index) };
        (atomic_entry & mask) != 0
    }

    #[inline]
    pub fn modify<const SET_BIT: bool>(&self, obj: usize) -> bool {
        unsafe {
            let offset = obj - self.heap_begin;
            let index = Self::offset_to_index(offset);
            let mask = Self::offset_to_mask(offset);
            let atomic_entry = &mut *self.bitmap_begin.add(index);
            let old_word = *atomic_entry;
            if SET_BIT {
                if (old_word & mask) == 0 {
                    *atomic_entry = old_word | mask;
                }
            } else {
                let before = *atomic_entry;
                *atomic_entry = old_word & !mask;

                //atomic_entry.store(old_word & !mask, Ordering::Relaxed);
            }

            (old_word & mask) != 0
        }
    }
    #[inline]
    pub fn set_heap_limit(&mut self, new_end: usize) {
        let new_size = Self::offset_to_index(new_end - self.heap_begin) * size_of::<usize>();
        if new_size < self.bitmap_size {
            self.bitmap_size = new_size;
        }
        self.heap_limit = new_end;
    }
    #[inline]
    pub fn clear_to_zeros(&mut self) {
        if !self.bitmap_begin.is_null() {
            unsafe {
                core::ptr::write_bytes(self.mem_map.as_ptr() as *mut u8, 0, self.mem_map.len());
            }
        }
    }

    #[inline]
    pub fn clear_range(&self, begin: usize, end: usize) {
        unsafe {
            let mut begin_offset = begin - self.heap_begin;
            let mut end_offset = end - self.heap_begin;

            while begin_offset < end_offset && Self::offset_bit_index(begin_offset) != 0 {
                self.clear(self.heap_begin + begin_offset);
                begin_offset += ALIGNMENT;
            }
            while begin_offset < end_offset && Self::offset_bit_index(end_offset) != 0 {
                end_offset -= ALIGNMENT;
                self.clear(self.heap_begin + end_offset);
            }

            let start_index = Self::offset_to_index(begin_offset);
            let end_index = Self::offset_to_index(end_offset);
            core::ptr::write_bytes(
                self.bitmap_begin.add(start_index).cast::<u8>(),
                0,
                (end_index - start_index) * size_of::<usize>(),
            );
        }
    }
    pub fn size(&self) -> usize {
        self.bitmap_size
    }

    pub fn heap_begin(&self) -> usize {
        self.heap_begin
    }

    pub fn heap_limit(&self) -> usize {
        self.heap_limit
    }
    pub fn begin(&self) -> *mut usize {
        self.bitmap_begin
    }
    pub fn set_heap_size(&mut self, bytes: usize) {
        self.heap_limit = self.heap_begin + bytes;
        self.bitmap_size = Self::offset_to_index(bytes) * size_of::<usize>();
    }

    #[allow(unused_braces)]
    #[inline]
    pub fn clear(&self, obj: usize) {
        self.modify::<{ false }>(obj);
    }

    #[allow(unused_braces)]
    #[inline]
    pub fn set(&self, obj: usize) {
        self.modify::<{ true }>(obj);
    }
}

impl<const ALIGN: usize> fmt::Debug for SpaceBitmap<ALIGN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[begin={:p},end={:p}]",
            self.heap_begin as *const (), self.heap_limit as *const ()
        )
    }
}
