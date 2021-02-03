use std::sync::atomic::Ordering;
pub const BITMAP_SIZE: usize = super::marked_block::ATOMS_PER_BLOCK;
pub const BITS_IN_WORD: usize = core::mem::size_of::<usize>() * 8;
pub const NUMBER_OF_WORDS: usize = (BITMAP_SIZE + BITS_IN_WORD - 1) / BITS_IN_WORD;

pub struct BitMap {
    bits: [usize; NUMBER_OF_WORDS],
}

impl BitMap {
    pub fn size(&self) -> usize {
        BITMAP_SIZE
    }
    #[inline]
    pub fn set(&mut self, n: usize) {
        self.bits[n / BITS_IN_WORD] |= 1 << (n % BITS_IN_WORD);
    }

    #[inline]
    pub fn set_val(&mut self, n: usize, val: bool) {
        if val {
            self.set(n);
        } else {
            self.clear(n);
        }
    }

    #[inline]
    pub fn clear(&mut self, n: usize) {
        self.bits[n / BITS_IN_WORD] &= !(1 << (n % BITS_IN_WORD));
    }

    #[inline]
    pub fn test_and_set(&mut self, n: usize) -> bool {
        let mask = 1 << (n % BITS_IN_WORD);
        let index = n / BITS_IN_WORD;
        let result = self.bits[index] & mask;
        self.bits[index] |= mask;
        result != 0
    }

    #[inline]
    pub fn test_and_clear(&mut self, n: usize) -> bool {
        let mask = 1 << (n % BITS_IN_WORD);
        let index = n / BITS_IN_WORD;
        let result = self.bits[index] & mask;
        self.bits[index] &= !mask;
        result != 0
    }

    pub fn is_zero(&self) -> bool {
        self.bits.iter().all(|x| *x == 0)
    }

    #[inline]
    pub fn concurrent_test_and_set(&self, n: usize) -> bool {
        let mask = 1 << (n % BITS_IN_WORD);
        let index = n / BITS_IN_WORD;
        let entry = unsafe {
            &*(&self.bits[index] as *const usize as *const std::sync::atomic::AtomicUsize)
        };
        loop {
            let old_value = entry.load(Ordering::Relaxed);
            let mut new_value = old_value;
            if new_value & mask != 0 {
                return false;
            }
            new_value &= !mask;
            match entry.compare_exchange_weak(
                old_value,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return true,
                Err(_) => {}
            }
        }
    }
    #[inline]
    pub fn concurrent_test_and_clear(&mut self, n: usize) -> bool {
        let mask = 1 << (n % BITS_IN_WORD);
        let index = n / BITS_IN_WORD;
        let entry = unsafe {
            &*(&self.bits[index] as *const usize as *const std::sync::atomic::AtomicUsize)
        };
        loop {
            let old_value = entry.load(Ordering::Relaxed);
            let mut new_value = old_value;
            if new_value & mask == 0 {
                return false;
            }
            new_value |= mask;
            if let Ok(_) = entry.compare_exchange_weak(
                old_value,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                return true;
            }
        }
    }
    pub fn get(&self, n: usize) -> bool {
        unsafe {
            let mask = 1 << (n % BITS_IN_WORD);
            let index = n / BITS_IN_WORD;
            let entry =
                &*(&self.bits[index] as *const usize as *const std::sync::atomic::AtomicUsize);

            (entry.load(Ordering::Relaxed) & mask) != 0
        }
    }
    pub fn clear_all(&mut self) {
        unsafe {
            core::ptr::write_bytes(self.bits.as_mut_ptr(), 0, self.bits.len());
        }
    }

    pub fn new() -> Self {
        Self {
            bits: [0; NUMBER_OF_WORDS],
        }
    }
}
