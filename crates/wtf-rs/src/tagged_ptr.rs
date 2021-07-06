use core::{mem::transmute, sync::atomic::AtomicU64};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaggedPointer<const TAG_BITS: usize> {
    value: u64,
}

impl<const BITS: usize> TaggedPointer<BITS> {
    pub const TAG_MASK: usize = BITS - 1;
    pub const PTR_MASK: usize = !Self::TAG_MASK;

    pub fn get_ptr(self) -> usize {
        (self.value & Self::PTR_MASK as u64) as _
    }

    pub fn tag(self) -> usize {
        (self.value & Self::TAG_MASK as u64) as _
    }

    pub fn new<T>(ptr: *const T, tag: usize) -> Self {
        Self {
            value: ptr as u64 | tag as u64,
        }
    }

    pub fn set_ptr<T>(&mut self, ptr: *const T) {
        self.value = (ptr as u64) | (self.value & Self::TAG_MASK as u64);
    }

    pub fn set_tag(&mut self, tag: usize) {
        self.value = self.get_ptr() as u64 | tag as u64;
    }
    fn as_atomic(&self) -> &AtomicU64 {
        unsafe { transmute(&self.value) }
    }
    pub fn compare_and_set_tag(&self, current: usize, new: usize) -> bool {
        let ptr = self.get_ptr();
        self.as_atomic()
            .compare_exchange_weak(
                ptr as u64 | current as u64,
                ptr as u64 | new as u64,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    }
}

impl<const BITS: usize> core::fmt::Debug for TaggedPointer<BITS> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TaggedPointer(tag:{},data:{:x})",
            self.tag(),
            self.get_ptr()
        )
    }
}
