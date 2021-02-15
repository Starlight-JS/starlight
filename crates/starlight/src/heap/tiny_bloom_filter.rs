#[derive(Copy, Clone)]
pub struct TinyBloomFilter {
    bits: usize,
}
impl TinyBloomFilter {
    pub const fn new(bits: usize) -> Self {
        Self { bits }
    }

    pub fn rule_out(&self, bits: usize) -> bool {
        if bits == 0 {
            return true;
        }
        if (bits & self.bits) != bits {
            return true;
        }
        false
    }

    pub fn add(&mut self, other: &Self) {
        self.bits |= other.bits;
    }

    pub fn add_bits(&mut self, bits: usize) {
        self.bits |= bits;
    }

    pub fn reset(&mut self) {
        self.bits = 0;
    }

    pub fn bits(&self) -> usize {
        self.bits
    }
}
