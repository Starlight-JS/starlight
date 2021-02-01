#[macro_export]
macro_rules! space_bitmap_gen {
    ($name : ident, $align: expr,$region_size: expr) => {
        pub struct $name {
            bitmap_: [usize; Self::BITMAP_SIZE / core::mem::size_of::<usize>()],
        }

        impl $name {
            pub fn clear_all(&mut self) {
                for byte in self.bitmap_.iter_mut() {
                    *byte = 0;
                }
            }
            pub fn is_empty(&self) -> bool {
                for byte in self.bitmap_.iter() {
                    if *byte != 0 {
                        return false;
                    }
                }
                true
            }
            /*#[inline]
            pub fn visit_marked_range(
                &self,
                heap_begin: usize,
                visit_begin: usize,
                visit_end: usize,
                mut visitor: impl FnMut(usize),
            ) {
                let offset_start = visit_begin - heap_begin;
                let offset_end = visit_end - heap_begin;

                let index_start = Self::offset_to_index(offset_start);
                let index_end = Self::offset_to_index(offset_end);

                let bit_start = (offset_start / $align) * (core::mem::size_of::<usize>() * 8);
                let bit_end = (offset_end / $align) * (core::mem::size_of::<usize>() * 8);

                let mut left_edge = self.bitmap_[index_start];

                left_edge &= !((1 << bit_start) - 1);

                let mut right_edge;
                if index_start < index_end {
                    if left_edge != 0 {
                        let ptr_base = Self::index_to_offset(index_start) as usize + heap_begin;
                        while {
                            let shift = left_edge.trailing_zeros() as usize;
                            let obj = ptr_base + shift * $align;
                            visitor(obj);
                            left_edge ^= 1 << shift;
                            left_edge != 0
                        } {}
                    }

                    for i in index_start + 1..index_end {
                        let mut w = self.bitmap_[i];
                        if w != 0 {
                            let ptr_base = Self::index_to_offset(i) as usize + heap_begin;
                            while {
                                let shift = w.trailing_zeros() as usize;
                                let obj = ptr_base + shift * $align;
                                visitor(obj);
                                w ^= 1 << shift;
                                w != 0
                            } {}
                        }
                    }

                    if bit_end == 0 {
                        right_edge = 0;
                    } else {
                        right_edge = self.bitmap_[index_end];
                    }
                } else {
                    right_edge = left_edge;
                }

                right_edge &= (1 << bit_end) - 1;

                if right_edge != 0 {
                    let ptr_base = Self::index_to_offset(index_end) as usize + heap_begin;
                    while {
                        let shift = right_edge.trailing_zeros() as usize;
                        let obj = ptr_base + shift * $align;
                        visitor(obj);
                        right_edge ^= 1 << shift;
                        right_edge != 0
                    } {}
                }
            }*/
            pub const BITMAP_SIZE: usize = {
                let bytes_covered_per_word = $align * (core::mem::size_of::<usize>() * 8);
                ($crate::heap::util::round_up($region_size, bytes_covered_per_word as _)
                    / bytes_covered_per_word as u64) as usize
                    * core::mem::size_of::<isize>()
            };
            pub const fn offset_bit_index(offset: usize) -> usize {
                (offset / $align) % (core::mem::size_of::<usize>() * 8)
            }

            pub const fn offset_to_index(offset: usize) -> usize {
                offset / $align / (core::mem::size_of::<usize>() * 8)
            }

            pub const fn index_to_offset(index: usize) -> isize {
                return index as isize
                    * $align as isize
                    * (core::mem::size_of::<usize>() as isize * 8);
            }

            pub const fn offset_to_mask(offset: usize) -> usize {
                1 << ((offset / $align) % (core::mem::size_of::<usize>() * 8))
            }
            #[inline(always)]
            pub fn test(&self, object: usize, heap_begin: usize) -> bool {
                let offset = object - heap_begin;

                let index = Self::offset_to_index(offset as _);
                let mask = Self::offset_to_mask(offset as _);
                let entry = self.bitmap_[index as usize];

                (entry & mask) != 0
            }

            #[inline(always)]
            pub fn set(&mut self, object: usize, heap_begin: usize) -> bool {
                let offset = object - heap_begin;
                let index = Self::offset_to_index(offset as _);
                let mask = Self::offset_to_mask(offset as _);
                let entry = &mut self.bitmap_[index as usize];
                if (*entry & mask) == 0 {
                    *entry |= mask;
                    return true;
                }
                false
            }
            #[inline(always)]
            pub fn clear(&mut self, object: usize, heap_begin: usize) -> bool {
                let offset = object - heap_begin;
                let index = Self::offset_to_index(offset as _);
                let mask = Self::offset_to_mask(offset as _);
                let entry = &mut self.bitmap_[index as usize];
                if (*entry & mask) != 0 {
                    *entry &= !mask;
                    return true;
                }
                false
            }
            #[inline(always)]
            pub fn new() -> Self {
                let b = [0usize; Self::BITMAP_SIZE / core::mem::size_of::<usize>()];

                let this = Self { bitmap_: b };
                this
            }
        }
    };
}
