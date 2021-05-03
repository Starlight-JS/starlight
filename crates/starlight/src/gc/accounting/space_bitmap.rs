/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use atomic::*;
use memmap2::MmapMut;
use std::mem::size_of;

use crate::gc::round_up;
pub struct SpaceBitmap<const ALIGNMENT: usize> {
    mem_map: MmapMut,
    bitmap_begin: *mut Atomic<usize>,
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
        (round_up(capacity, bytes_covered_per_word as _) / bytes_covered_per_word as u64) as usize
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
            bitmap_begin: bitmap_begin.cast::<Atomic<usize>>(),

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
            //name: name.to_owned(),
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

    /// Sweep walk through space between `sweep_begin` and `sweep_end`.
    ///
    /// # Safety
    /// sweep_begin and sweep_end should be part of gc where bitmap is intended to use.
    pub unsafe fn sweep_walk(
        live_bitmap: &Self,
        mark_bitmap: &Self,
        sweep_begin: usize,
        sweep_end: usize,
        mut callback: impl FnMut(usize, usize),
    ) {
        if sweep_end <= sweep_begin {
            return;
        }

        let buffer_size = size_of::<isize>() * (size_of::<isize>() * 8);

        let live = live_bitmap.bitmap_begin;
        let mark = mark_bitmap.bitmap_begin;

        let start = Self::offset_to_index(sweep_begin - live_bitmap.heap_begin as usize);
        let end = Self::offset_to_index(sweep_end - live_bitmap.heap_begin - 1);
        let mut pointer_buf = vec![0usize; buffer_size];

        //let mut pointer_buf = alloc::vec::Vec::with![0usize; buffer_size];
        let mut cur_pointer: *mut usize = &mut pointer_buf[0];
        let pointer_end = cur_pointer.add(buffer_size - (size_of::<usize>() * 8));

        for i in start..=end {
            let mut garbage = (&*live.add(i as _)).load(Ordering::Relaxed)
                & !((&*mark.add(i as _)).load(Ordering::Relaxed));

            if garbage != 0 {
                let ptr_base = Self::index_to_offset(i) + live_bitmap.heap_begin as isize;
                let ptr_base = ptr_base as usize;
                while {
                    let shift = garbage.trailing_zeros() as usize;
                    garbage ^= 1 << shift;
                    cur_pointer.write(ptr_base + shift * ALIGNMENT);
                    cur_pointer = cur_pointer.offset(1);
                    garbage != 0
                } {}
                if cur_pointer >= &mut pointer_buf[buffer_size - (size_of::<usize>() * 8)] {
                    callback(
                        cur_pointer as usize - &pointer_buf[0] as *const _ as usize,
                        pointer_buf.as_ptr() as usize,
                    );
                    cur_pointer = pointer_buf.as_ptr() as *mut _;
                }
            }
        }

        if cur_pointer >= &mut pointer_buf[0] {
            callback(
                cur_pointer as usize - pointer_buf.as_ptr() as usize,
                pointer_buf.as_ptr() as usize,
            );
        }
        drop(pointer_buf);
    }
    #[inline]
    pub fn atomic_test_and_set(&self, object: usize) -> bool {
        unsafe {
            let offset = object as isize - self.heap_begin as isize;
            let index = Self::offset_to_index(offset as _);
            let mask = Self::offset_to_mask(offset as _);
            let atomic_entry = &*self.bitmap_begin.add(index as _);
            let mut old_word;
            while {
                old_word = atomic_entry.load(Ordering::Relaxed);
                if (old_word & mask) != 0 {
                    return true;
                }
                atomic_entry.compare_exchange_weak(
                    old_word,
                    old_word | mask,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) != Ok(old_word)
            } {}

            false
        }
    }
    #[inline]
    pub fn test(&self, object: usize) -> bool {
        let offset = object as isize - self.heap_begin as isize;
        let index = Self::offset_to_index(offset as _);
        let mask = Self::offset_to_mask(offset as _);
        let atomic_entry = unsafe { &*self.bitmap_begin.add(index as _) };
        (atomic_entry.load(Ordering::Relaxed) & mask) != 0
    }

    pub fn visit_marked_range(
        &self,
        visit_begin: usize,
        visit_end: usize,
        mut visitor: impl FnMut(usize),
    ) {
        unsafe {
            let offset_start = visit_begin - self.heap_begin;
            let offset_end = visit_end - self.heap_begin;

            let index_start = Self::offset_to_index(offset_start);
            let index_end = Self::offset_to_index(offset_end);

            let bit_start = (offset_start / ALIGNMENT) * (size_of::<usize>() * 8);
            let bit_end = (offset_end / ALIGNMENT) * (size_of::<usize>() * 8);

            let mut left_edge = self
                .bitmap_begin
                .add(index_start as _)
                .cast::<usize>()
                .read();

            left_edge &= !((1 << bit_start) - 1);

            let mut right_edge;
            if index_start < index_end {
                if left_edge != 0 {
                    let ptr_base = Self::index_to_offset(index_start) as usize + self.heap_begin;
                    while {
                        let shift = left_edge.trailing_zeros() as usize;
                        let obj = ptr_base + shift * ALIGNMENT;
                        visitor(obj);
                        left_edge ^= 1 << shift;
                        left_edge != 0
                    } {}
                }

                for i in index_start + 1..index_end {
                    let mut w = (&*self.bitmap_begin.add(i as _)).load(Ordering::Relaxed);
                    if w != 0 {
                        let ptr_base = Self::index_to_offset(i) as usize + self.heap_begin;
                        while {
                            let shift = w.trailing_zeros() as usize;
                            let obj = ptr_base + shift * ALIGNMENT;
                            visitor(obj);
                            w ^= 1 << shift;
                            w != 0
                        } {}
                    }
                }

                if bit_end == 0 {
                    right_edge = 0;
                } else {
                    right_edge = self.bitmap_begin.add(index_end as _).cast::<usize>().read();
                }
            } else {
                right_edge = left_edge;
            }

            right_edge &= (1usize.wrapping_shl(bit_end as u32)) - 1;

            if right_edge != 0 {
                let ptr_base = Self::index_to_offset(index_end) as usize + self.heap_begin;
                while {
                    let shift = right_edge.trailing_zeros() as usize;
                    let obj = ptr_base + shift * ALIGNMENT;
                    visitor(obj);
                    right_edge ^= 1 << shift;
                    right_edge != 0
                } {}
            }
        }
    }

    pub fn walk(&self, mut visitor: impl FnMut(usize)) {
        unsafe {
            let end = Self::offset_to_index(self.heap_limit - self.heap_begin - 1);
            let bitmap_begin = self.bitmap_begin;
            for i in 0..=end {
                let mut w = (&*bitmap_begin.add(i as _)).load(Ordering::Relaxed);
                if w != 0 {
                    let ptr_base = Self::index_to_offset(i) as usize + self.heap_begin;

                    while {
                        let shift = w.trailing_zeros() as usize;
                        let obj = ptr_base + shift * ALIGNMENT;
                        visitor(obj);
                        w ^= 1 << shift;
                        w != 0
                    } {}
                }
            }
        }
    }
    #[inline]
    pub fn modify<const SET_BIT: bool>(&self, obj: usize) -> bool {
        unsafe {
            let offset = obj - self.heap_begin;
            let index = Self::offset_to_index(offset);
            let mask = Self::offset_to_mask(offset);
            let atomic_entry = &*self.bitmap_begin.add(index as _);
            let old_word = atomic_entry.load(Ordering::Relaxed);
            if SET_BIT {
                if (old_word & mask) == 0 {
                    atomic_entry.store(old_word | mask, Ordering::Relaxed);
                }
            } else {
                atomic_entry.store(old_word & !mask, Ordering::Relaxed);
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
                self.bitmap_begin.add(start_index as _).cast::<u8>(),
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
    pub fn begin(&self) -> *mut Atomic<usize> {
        self.bitmap_begin
    }
    pub fn set_heap_size(&mut self, bytes: usize) {
        self.heap_limit = self.heap_begin + bytes;
        self.bitmap_size = Self::offset_to_index(bytes) * size_of::<usize>();
    }

    pub fn copy_from(&self, source_bitmap: &Self) {
        let count = source_bitmap.size() / size_of::<usize>();
        unsafe {
            let src = source_bitmap.begin();
            let dest = self.begin();
            for i in 0..count {
                (&*dest.add(i as _)).store(
                    (&*src.add(i as _)).load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
            }
        }
    }

    #[allow(unused_braces)]
    #[inline]
    pub fn clear(&self, obj: usize) -> bool {
        self.modify::<{ false }>(obj)
    }

    #[allow(unused_braces)]
    #[inline]
    pub fn set(&self, obj: usize) -> bool {
        self.modify::<{ true }>(obj)
    }
}
