use crate::cryptographically_random_number::cryptographically_random_number;

/// The code used to generate random numbers are inlined manually in JIT code.
/// So it needs to stay in sync with the JIT one.
pub struct WeakRandom {
    seed: u32,
    low: u64,
    high: u64,
}

impl WeakRandom {
    pub const fn next_state(mut x: u64, y: u64) -> u64 {
        x ^= x << 23;
        x ^= x >> 17;
        x ^= y ^ (y >> 26);
        x
    }
    fn advance(&mut self) -> u64 {
        let x = self.low;
        let y = self.high;
        self.low = y;
        self.high = Self::next_state(x, y);
        self.high + self.low
    }

    pub fn generate(mut seed: u32) -> u64 {
        if seed == 0 {
            seed = 1;
        }
        let low = seed as u64;
        let mut high = seed as u64;
        high = Self::next_state(low, high);
        low + high
    }

    pub fn low_offset() -> usize {
        object_offsetof!(Self, low)
    }

    pub fn high_offset() -> usize {
        object_offsetof!(Self, high)
    }
    pub fn get_u32(&mut self) -> u32 {
        self.advance() as _
    }
    pub fn get_u32_with_limit(&mut self, limit: u32) -> u32 {
        if limit <= 1 {
            return 0;
        }
        let cutoff = (u32::MAX as u64 + 1) / limit as u64 * limit as u64;
        loop {
            let value = self.get_u32();
            if value as u64 >= cutoff {
                continue;
            }
            return value % limit;
        }
    }

    pub fn get(&mut self) -> f64 {
        let value = self.advance() & ((1 << 53) - 1);
        value as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }
    pub fn set_seed(&mut self, mut seed: u32) {
        self.seed = seed;
        if seed == 0 {
            seed = 1;
        }
        self.low = seed as _;
        self.high = seed as _;
        self.advance();
    }
    pub const fn const_new() -> Self {
        let mut this = Self {
            low: 0,
            high: 0,
            seed: 0,
        };
        this.low = 1;
        this.high = 1;
        let x = this.low;
        let y = this.high;
        this.low = y;
        this.high = Self::next_state(x, y);
        this
    }
    pub fn new(seed: Option<u32>) -> Self {
        let mut this = Self {
            low: 0,
            high: 0,
            seed: 0,
        };
        this.set_seed(seed.unwrap_or_else(cryptographically_random_number));
        this
    }
}
