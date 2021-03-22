use crate::heap::{
    cell::{GcCell, GcPointer, GcPointerBase, Trace, Tracer, WeakRef, WeakSlot, DEFINETELY_WHITE},
    snapshot::{
        deserializer::Deserializable,
        deserializer::Deserializer,
        serializer::{Serializable, SnapshotSerializer},
    },
    MarkingConstraint,
};
use crate::vm::Runtime;
use std::{cmp::Ordering, fmt};
use std::{
    mem::size_of,
    ptr::{null_mut, NonNull},
};

pub const K: usize = 1024;
pub mod accounting;
pub mod bump;
pub mod freelist;
pub mod mark_copy;
pub mod mem;
pub mod os;
pub mod safepoint;
#[macro_use]
pub mod shadowstack;

pub const fn round_down(x: u64, n: u64) -> u64 {
    x & !n
}

pub const fn round_up(x: u64, n: u64) -> u64 {
    round_down(x + n - 1, n)
}

pub struct GcStats {
    pub allocated: usize,
    pub threshold: usize,
}

pub trait GarbageCollector {
    fn allocate(&mut self, size: usize, vtable: usize) -> Option<NonNull<GcPointerBase>>;
    fn gc(&mut self);
    fn collect_if_necessary(&mut self);
    fn stats(&self) -> GcStats;
    fn add_constraint(&mut self, contraint: Box<dyn MarkingConstraint>);
    fn make_weak_slot(&mut self, base: *mut GcPointerBase) -> *mut WeakSlot;
    fn walk(&mut self, callback: &mut dyn FnMut(*mut GcPointerBase, usize));
}

pub struct Heap {
    gc: Box<dyn GarbageCollector>,
}

impl Heap {
    pub fn make_null_weak<T: GcCell>(&mut self) -> WeakRef<T> {
        let slot = self.gc.make_weak_slot(null_mut());
        unsafe {
            let weak = WeakRef {
                inner: NonNull::new_unchecked(slot),
                marker: Default::default(),
            };
            weak
        }
    }

    pub fn make_weak<T: GcCell>(&mut self, p: GcPointer<T>) -> WeakRef<T> {
        let slot = self.gc.make_weak_slot(p.base.as_ptr());
        unsafe {
            let weak = WeakRef {
                inner: NonNull::new_unchecked(slot),
                marker: Default::default(),
            };
            weak
        }
    }

    pub fn make_weak_slo(&mut self, p: *mut GcPointerBase) -> *mut WeakSlot {
        self.gc.make_weak_slot(p)
    }
}

pub struct FreeObject {
    size: usize,
}

impl GcCell for FreeObject {
    fn compute_size(&self) -> usize {
        self.size
    }
    fn deser_pair(&self) -> (usize, usize) {
        unreachable!()
    }
}

unsafe impl Trace for FreeObject {
    fn trace(&mut self, _visitor: &mut dyn Tracer) {
        unreachable!();
    }
}
impl Serializable for FreeObject {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {
        unreachable!()
    }
}

impl Deserializable for FreeObject {
    unsafe fn deserialize(_at: *mut u8, _deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn deserialize_inplace(_deser: &mut Deserializer) -> Self {
        unreachable!()
    }
    unsafe fn dummy_read(_deser: &mut Deserializer) {
        unreachable!()
    }

    unsafe fn allocate(_rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }
}

pub unsafe fn fill_with_free(from: usize, to: usize) {
    let mut scan = from;
    while scan < to {
        let addr = scan as *mut GcPointerBase;
        (*addr).vtable = 0;
        (*addr)
            .cell_state
            .store(DEFINETELY_WHITE, std::sync::atomic::Ordering::Relaxed);
        scan += size_of::<GcPointerBase>();
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address(usize);

impl Address {
    #[inline(always)]
    pub fn align_page(self) -> Address {
        mem::page_align(self.to_usize()).into()
    }

    #[inline(always)]
    pub fn align_page_down(self) -> Address {
        Address(self.0 & !(os::page_size() - 1))
    }

    #[inline(always)]
    pub fn is_page_aligned(self) -> bool {
        mem::is_page_aligned(self.to_usize())
    }
    #[inline(always)]
    pub fn from(val: usize) -> Address {
        Address(val)
    }

    #[inline(always)]
    pub fn region_start(self, size: usize) -> Region {
        Region::new(self, self.offset(size))
    }

    #[inline(always)]
    pub fn offset_from(self, base: Address) -> usize {
        debug_assert!(self >= base);

        self.to_usize() - base.to_usize()
    }

    #[inline(always)]
    pub fn offset(self, offset: usize) -> Address {
        Address(self.0 + offset)
    }

    #[inline(always)]
    pub fn sub(self, offset: usize) -> Address {
        Address(self.0 - offset)
    }

    #[inline(always)]
    pub fn add_ptr(self, words: usize) -> Address {
        Address(self.0 + words * std::mem::size_of::<usize>())
    }

    #[inline(always)]
    pub fn sub_ptr(self, words: usize) -> Address {
        Address(self.0 - words * std::mem::size_of::<usize>())
    }

    #[inline(always)]
    pub fn to_usize(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub fn from_ptr<T>(ptr: *const T) -> Address {
        Address(ptr as usize)
    }

    #[inline(always)]
    pub fn to_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    #[inline(always)]
    pub fn to_mut_ptr<T>(&self) -> *mut T {
        self.0 as *const T as *mut T
    }

    #[inline(always)]
    pub fn null() -> Address {
        Address(0)
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub fn is_non_null(self) -> bool {
        self.0 != 0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:x}", self.to_usize())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:x}", self.to_usize())
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

#[derive(Copy, Clone)]
pub struct Region {
    pub start: Address,
    pub end: Address,
}

impl Region {
    pub fn new(start: Address, end: Address) -> Region {
        debug_assert!(start <= end);

        Region { start, end }
    }

    #[inline(always)]
    pub fn contains(&self, addr: Address) -> bool {
        self.start <= addr && addr < self.end
    }

    #[inline(always)]
    pub fn valid_top(&self, addr: Address) -> bool {
        self.start <= addr && addr <= self.end
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.end.to_usize() - self.start.to_usize()
    }

    #[inline(always)]
    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    #[inline(always)]
    pub fn disjunct(&self, other: &Region) -> bool {
        self.end <= other.start || self.start >= other.end
    }

    #[inline(always)]
    pub fn overlaps(&self, other: &Region) -> bool {
        !self.disjunct(other)
    }

    #[inline(always)]
    pub fn fully_contains(&self, other: &Region) -> bool {
        self.contains(other.start) && self.valid_top(other.end)
    }
}

impl Default for Region {
    fn default() -> Region {
        Region {
            start: Address::null(),
            end: Address::null(),
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

pub struct FormattedSize {
    pub size: usize,
}

impl fmt::Display for FormattedSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ksize = (self.size as f64) / 1024f64;

        if ksize < 1f64 {
            return write!(f, "{}B", self.size);
        }

        let msize = ksize / 1024f64;

        if msize < 1f64 {
            return write!(f, "{:.1}K", ksize);
        }

        let gsize = msize / 1024f64;

        if gsize < 1f64 {
            write!(f, "{:.1}M", msize)
        } else {
            write!(f, "{:.1}G", gsize)
        }
    }
}

pub fn formatted_size(size: usize) -> FormattedSize {
    FormattedSize { size }
}
