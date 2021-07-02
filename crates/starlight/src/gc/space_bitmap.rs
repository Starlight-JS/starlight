use super::*;
use crate::gc::round_up;
use atomic::{Atomic, Ordering};
use core::fmt;
use core::mem::size_of;
use memmap2::MmapMut;

pub struct SpaceBitmap<const ALIGN: usize> {
    mem_map: MmapMut,
    bitmap_begin: *mut Atomic<usize>,
    bitmap_size: usize,
    heap_begin: usize,
    heap_limit: usize,
    name: &'static str,
}
const BITS_PER_INTPTR: usize = size_of::<usize>() * 8;
impl<const ALIGN: usize> SpaceBitmap<ALIGN> {
    #[inline]
    pub fn get_name(&self) -> &'static str {
        self.name
    }
    #[inline]
    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }
    #[inline]
    pub fn heap_limit(&self) -> usize {
        self.heap_limit
    }
    #[inline]
    pub fn heap_begin(&self) -> usize {
        self.heap_begin
    }
    #[inline]
    pub fn set_heap_size(&mut self, bytes: usize) {
        let limit = self.heap_begin + bytes;
        self.bitmap_size = Self::offset_to_index(bytes) * size_of::<usize>();
        assert_eq!(self.heap_size(), bytes);
    }
    #[inline]
    pub fn heap_size(&self) -> usize {
        Self::index_to_offset(self.size() as u64 / size_of::<usize>() as u64) as _
    }
    #[inline]
    pub fn has_address(&self, obj: *const u8) -> bool {
        let offset = (obj as usize).wrapping_sub(self.heap_begin);
        let index = Self::offset_to_index(offset);
        index < (self.bitmap_size / size_of::<usize>())
    }
    #[inline]
    pub fn size(&self) -> usize {
        self.bitmap_size
    }
    #[inline]
    pub fn begin(&self) -> *mut Atomic<usize> {
        self.bitmap_begin
    }
    #[inline]
    pub fn index_to_offset(index: u64) -> u64 {
        index * ALIGN as u64 * BITS_PER_INTPTR as u64
    }
    #[inline]
    pub fn offset_to_index(offset: usize) -> usize {
        offset / ALIGN / BITS_PER_INTPTR
    }
    #[inline]
    pub fn offset_bit_index(offset: usize) -> usize {
        (offset / ALIGN) % BITS_PER_INTPTR
    }
    #[inline]
    pub fn offset_to_mask(offset: usize) -> usize {
        1 << Self::offset_bit_index(offset)
    }
    #[inline]
    pub fn atomic_test_and_set(&self, obj: *const u8) -> bool {
        let addr = obj as usize;
        debug_assert!(addr >= self.heap_begin);
        let offset = addr.wrapping_sub(self.heap_begin);
        let index = Self::offset_to_index(offset);
        let mask = Self::offset_to_mask(offset);
        unsafe {
            let atomic_entry = &mut *self.bitmap_begin.add(index);
            debug_assert!(
                index < self.bitmap_size / size_of::<usize>(),
                "bitmap_size: {}",
                self.bitmap_size
            );

            let mut old_word;
            while {
                old_word = atomic_entry.load(Ordering::Relaxed);
                if (old_word & mask) != 0 {
                    return true;
                }
                !atomic_entry
                    .compare_exchange_weak(
                        old_word,
                        old_word | mask,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            } {}

            false
        }
    }
    #[inline]
    pub fn test(&self, obj: *const u8) -> bool {
        let addr = obj as usize;
        debug_assert!(self.has_address(obj), "Invalid object address: {:p}", obj);
        debug_assert!(self.heap_begin <= addr);
        unsafe {
            let offset = addr.wrapping_sub(self.heap_begin);
            let index = Self::offset_to_index(offset);
            ((*self.bitmap_begin.add(index)).load(Ordering::Relaxed) & Self::offset_to_mask(offset))
                != 0
        }
    }
    #[inline]
    pub fn modify<const SET_BIT: bool>(&self, obj: *const u8) -> bool {
        let addr = obj as usize;
        debug_assert!(addr >= self.heap_begin);
        debug_assert!(self.has_address(obj), "Invalid object address: {:p}", obj);
        let offset = addr.wrapping_sub(self.heap_begin);
        let index = Self::offset_to_index(offset);
        let mask = Self::offset_to_mask(offset);
        debug_assert!(
            index < self.bitmap_size / size_of::<usize>(),
            "bitmap size: {}",
            self.bitmap_size
        );
        let atomic_entry = unsafe { &*self.bitmap_begin.add(index) };
        let old_word = atomic_entry.load(Ordering::Relaxed);
        if SET_BIT {
            // Check the bit before setting the word incase we are trying to mark a read only bitmap
            // like an image space bitmap. This bitmap is mapped as read only and will fault if we
            // attempt to change any words. Since all of the objects are marked, this will never
            // occur if we check before setting the bit. This also prevents dirty pages that would
            // occur if the bitmap was read write and we did not check the bit.
            if (old_word & mask) == 0 {
                atomic_entry.store(old_word | mask, Ordering::Relaxed);
            }
        } else {
            atomic_entry.store(old_word & !mask, Ordering::Relaxed);
        }

        debug_assert_eq!(self.test(obj), SET_BIT);
        (old_word & mask) != 0
    }

    #[inline(always)]
    pub fn set(&self, obj: *const u8) -> bool {
        self.modify::<true>(obj)
    }

    #[inline(always)]
    pub fn clear(&self, obj: *const u8) -> bool {
        self.modify::<false>(obj)
    }

    pub fn compute_bitmap_size(capacity: u64) -> usize {
        let bytes_covered_per_word = ALIGN * BITS_PER_INTPTR;
        ((round_up(capacity, bytes_covered_per_word as _) / bytes_covered_per_word as u64)
            * size_of::<usize>() as u64) as _
    }
    pub fn compute_heap_size(bitmap_bytes: u64) -> usize {
        (bitmap_bytes * 8 * ALIGN as u64) as _
    }
    pub fn new(
        name: &'static str,
        mem_map: MmapMut,
        bitmap_begin: *mut usize,
        bitmap_size: usize,
        heap_begin: *mut u8,
        heap_capacity: usize,
    ) -> Self {
        Self {
            name,
            mem_map,
            bitmap_size,
            bitmap_begin: bitmap_begin.cast(),

            heap_begin: heap_begin as _,
            heap_limit: heap_begin as usize + heap_capacity,
        }
    }

    pub fn create_from_memmap(
        name: &'static str,
        mem_map: MmapMut,
        heap_begin: *mut u8,
        heap_capacity: usize,
    ) -> Self {
        let bitmap_begin = mem_map.as_ptr() as *mut u8;
        let bitmap_size = Self::compute_bitmap_size(heap_capacity as _);
        Self {
            name,
            mem_map,
            bitmap_begin: bitmap_begin.cast(),
            bitmap_size,
            heap_begin: heap_begin as _,
            heap_limit: heap_begin as usize + heap_capacity,
        }
    }

    pub fn create(name: &'static str, heap_begin: *mut u8, heap_capacity: usize) -> Self {
        let bitmap_size = Self::compute_bitmap_size(heap_capacity as _);

        let mem_map = MmapMut::map_anon(bitmap_size).unwrap();
        Self::create_from_memmap(name, mem_map, heap_begin, heap_capacity)
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
