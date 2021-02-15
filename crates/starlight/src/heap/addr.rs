use core::cmp::Ordering;
/// Wrapper around usize for easy pointer math.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address(usize);

impl Address {
    /// Construct Self from usize
    #[inline(always)]
    pub fn from(val: usize) -> Address {
        Address(val)
    }
    /// Offset from `base`
    #[inline(always)]
    pub fn offset_from(self, base: Address) -> usize {
        debug_assert!(self >= base);

        self.to_usize() - base.to_usize()
    }
    /// Return self + offset
    #[inline(always)]
    pub fn offset(self, offset: usize) -> Address {
        Address(self.0 + offset)
    }
    /// Return self - offset
    #[inline(always)]
    pub fn sub(self, offset: usize) -> Address {
        Address(self.0 - offset)
    }
    /// Add pointer to self.
    #[inline(always)]
    pub fn add_ptr(self, words: usize) -> Address {
        Address(self.0 + words * core::mem::size_of::<usize>())
    }
    /// Sub pointer to self
    #[inline(always)]
    pub fn sub_ptr(self, words: usize) -> Address {
        Address(self.0 - words * core::mem::size_of::<usize>())
    }
    /// Convert pointer to usize
    #[inline(always)]
    pub const fn to_usize(self) -> usize {
        self.0
    }
    /// Construct from pointer
    #[inline(always)]
    pub fn from_ptr<T>(ptr: *const T) -> Address {
        Address(ptr as usize)
    }
    /// Convert to *const T
    #[inline(always)]
    pub fn to_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    /// Convert to *mut T
    #[inline(always)]
    pub fn to_mut_ptr<T>(&self) -> *mut T {
        self.0 as *const T as *mut T
    }
    /// Create null pointer
    #[inline(always)]
    pub fn null() -> Address {
        Address(0)
    }
    /// Check if self is null
    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
    /// Check if self is non null
    #[inline(always)]
    pub fn is_non_null(self) -> bool {
        self.0 != 0
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Address) -> Ordering {
        self.to_usize().cmp(&other.to_usize())
    }
}

impl From<usize> for Address {
    fn from(val: usize) -> Address {
        Address(val)
    }
}
/// Rounds up `x` to multiple of `divisor`
pub const fn round_up_to_multiple_of(divisor: usize, x: usize) -> usize {
    (x + (divisor - 1)) & !(divisor - 1)
}
