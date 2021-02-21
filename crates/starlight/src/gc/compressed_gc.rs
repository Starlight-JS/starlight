use core::{
    alloc::Layout,
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::{null_mut, NonNull},
    sync::atomic::{AtomicBool, Ordering},
};
#[cfg(feature = "compress-ptr-16")]
use std::num::NonZeroU16;

use linked_list_allocator::Heap as LHeap;
use memmap2::MmapMut;
#[cfg(feature = "compress-ptr-32")]
pub type NonZeroCompressedInternal = std::num::NonZeroU32;
#[cfg(feature = "compress-ptr-16")]
pub type NonZeroCompressedInternal = NonZeroU16;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroCompressed(NonZeroCompressedInternal);

impl std::fmt::Pointer for NonZeroCompressed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

impl NonZeroCompressed {
    pub fn as_ptr<T>(self) -> *mut T {
        decompress_ptr(self.0.get()).cast()
    }

    pub fn get(self) -> Compressed {
        self.0.get()
    }

    pub unsafe fn new_unchecked(x: Compressed) -> Self {
        Self(NonZeroCompressedInternal::new_unchecked(x))
    }

    pub fn new(x: Compressed) -> Option<Self> {
        Some(Self(NonZeroCompressedInternal::new(x)?))
    }
}
#[cfg(feature = "compress-ptr-16")]
pub type Compressed = u16;
#[cfg(not(feature = "compress-ptr-16"))]
pub type Compressed = u32;

struct SpinLock(AtomicBool);
impl SpinLock {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }
    #[inline]
    pub fn lock(&self) {
        loop {
            match self
                .0
                .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                _ => {
                    core::hint::spin_loop();
                    continue;
                }
            }
        }
    }
    #[inline]
    pub fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct StarliteHeap {
    map: MmapMut,
    size: usize,
    base: usize,
    alloc: LHeap,
    allocated: usize,
    lock: SpinLock,
}
impl StarliteHeap {
    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        {
            self.lock.lock();
            let p = self.alloc.allocate_first_fit(layout);
            self.allocated += layout.size();
            self.lock.unlock();
            p.map(|x| x.as_ptr()).unwrap_or_else(|_| null_mut())
        }
    }

    pub fn free(&mut self, p: *mut u8, layout: Layout) {
        unsafe {
            self.lock.lock();
            self.alloc.deallocate(NonNull::new_unchecked(p), layout);
            self.allocated -= layout.size();
            self.lock.unlock();
        }
    }
}
#[cfg(not(feature = "compress-ptr-16"))]
static mut HEAP_SIZE: usize = 4 * 1024 * 1024 * 1024;
#[cfg(feature = "compress-ptr-16")]
static mut HEAP_SIZE: usize = 512 * 1024;
unsafe impl Send for StarliteHeap {}
unsafe impl Sync for StarliteHeap {}

struct Handle(UnsafeCell<StarliteHeap>);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
static STARLITE_HEAP: once_cell::sync::Lazy<Handle> = once_cell::sync::Lazy::new(|| unsafe {
    let mut map = MmapMut::map_anon(HEAP_SIZE).expect("failed to initialize map");
    let heap = LHeap::new(map.as_mut_ptr().add(16) as _, HEAP_SIZE - 16);
    Handle(UnsafeCell::new(StarliteHeap {
        alloc: heap,
        allocated: 0,
        base: map.as_mut_ptr() as _,
        size: HEAP_SIZE - 16,
        lock: SpinLock::new(),
        map,
    }))
});

pub fn heap() -> &'static mut StarliteHeap {
    unsafe { &mut *(*STARLITE_HEAP).0.get() }
}

pub struct SBox<T> {
    value: NonNull<T>,
}
pub fn heap_allocated() -> usize {
    heap().allocated
}
impl<T> SBox<T> {
    pub fn new(value: T) -> Self {
        let memory = heap()
            .alloc(Layout::new::<T>().align_to(16).unwrap())
            .cast::<T>();
        unsafe {
            memory.write(value);
            Self {
                value: NonNull::new_unchecked(memory),
            }
        }
    }

    pub fn into_raw(this: Self) -> *mut T {
        let p = this.value.as_ptr();
        core::mem::forget(this);
        p
    }

    pub unsafe fn from_raw(mem: *mut T) -> Self {
        {
            Self {
                value: NonNull::new_unchecked(mem),
            }
        }
    }

    pub fn compress(self) -> CompressedBox<T> {
        let heap = heap();

        let compressed = self.value.as_ptr() as isize - heap.base as isize;

        core::mem::forget(self);
        unsafe {
            CompressedBox {
                marker: Default::default(),
                compressed_ptr: NonZeroCompressed::new_unchecked(compressed as Compressed),
            }
        }
    }
}

impl<T> Deref for SBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value.as_ptr() }
    }
}

impl<T> DerefMut for SBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value.as_ptr() }
    }
}

pub struct CompressedBox<T> {
    compressed_ptr: NonZeroCompressed,
    marker: PhantomData<T>,
}

impl<T> CompressedBox<T> {
    pub fn raw(&self) -> NonZeroCompressed {
        self.compressed_ptr
    }

    pub fn new(value: T) -> Self {
        SBox::new(value).compress()
    }
    pub unsafe fn from_raw(x: Compressed) -> Self {
        Self {
            compressed_ptr: NonZeroCompressed::new(x).expect("null pointer"),
            marker: Default::default(),
        }
    }
    fn get_decompressed_ptr(&self) -> *mut T {
        let heap = heap();
        let p = (heap.base as isize + self.compressed_ptr.get() as isize) as *mut T;

        p
    }
    pub fn into_raw(this: Self) -> Compressed {
        let p = this.raw().get();
        core::mem::forget(this);
        p
    }
    pub fn decompress(self) -> SBox<T> {
        let p = self.get_decompressed_ptr();
        unsafe { SBox::from_raw(p) }
    }
}

impl<T> Drop for CompressedBox<T> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.get_decompressed_ptr());
            heap().free(self.get_decompressed_ptr().cast(), Layout::new::<T>());
        }
    }
}
impl<T> Drop for SBox<T> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.value.as_ptr());
            heap().free(self.value.as_ptr().cast(), Layout::new::<T>());
        }
    }
}
impl<T> Deref for CompressedBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.get_decompressed_ptr() }
    }
}

impl<T> DerefMut for CompressedBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.get_decompressed_ptr() }
    }
}

struct RcInner<T> {
    value: T,
    rc: u32,
}
pub fn decompress_ptr(x: Compressed) -> *mut u8 {
    let heap = heap();
    let p = (heap.base as isize + x as isize) as *mut u8;

    p
}

pub fn compress_ptr(x: *mut u8) -> Compressed {
    let heap = heap();
    let p = (x as isize - heap.base as isize) as Compressed;

    p
}
pub struct Rc<T> {
    ptr: NonZeroCompressed,
    _marker: PhantomData<T>,
}

impl<T> Rc<T> {
    pub fn into_raw(x: Self) -> Compressed {
        x.ptr.get()
    }
    pub unsafe fn from_raw(p: Compressed) -> Self {
        Self {
            ptr: NonZeroCompressed::new(p).expect("null"),
            _marker: Default::default(),
        }
    }
    fn inner(&self) -> &mut RcInner<T> {
        unsafe { &mut *decompress_ptr(self.ptr.get()).cast::<RcInner<T>>() }
    }

    pub fn new(value: T) -> Self {
        let this = Self {
            ptr: unsafe {
                NonZeroCompressed::new_unchecked(CompressedBox::into_raw(CompressedBox::new(
                    RcInner { value, rc: 1 },
                )))
            },
            _marker: Default::default(),
        };
        debug_assert!(this.inner().rc == 1);

        this
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        self.inner().rc += 1;
        Self {
            ptr: self.ptr,
            _marker: Default::default(),
        }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        #[cold]
        #[inline]
        unsafe fn drop_rc<T>(this: &mut Rc<T>) {
            let _ = CompressedBox::<RcInner<T>>::from_raw(this.ptr.get());
        }
        if self.inner().rc == 1 {
            unsafe {
                drop_rc(self);
                return;
            }
        }

        self.inner().rc -= 1;
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner().value
    }
}

impl<T> DerefMut for Rc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner().value
    }
}

use std::{collections::VecDeque, mem::size_of};

use hashbrown::HashSet;

use crate::heap::addr::{round_up_to_multiple_of, Address};

use super::{
    cell::{object_ty_of, Cell, Gc, Header, Tracer, GC_MARKED, GC_UNMARKED},
    constraint::MarkingConstraint,
    handle::HandleTrait,
};

pub struct Heap {
    constraints: Vec<Box<dyn MarkingConstraint>>,

    ndefers: u32,
    max_heap_size: usize,
    allocated: usize,
    pub(super) handles: HashSet<*mut dyn HandleTrait>,
    list: *mut Header,
}

impl Heap {
    pub fn new() -> Box<Self> {
        let mut this = Box::new(Self {
            constraints: vec![],
            handles: Default::default(),
            list: null_mut(),
            ndefers: 0,
            max_heap_size: 64 * 1024,
            allocated: 0,
        });
        this.add_core_constraints();
        this.init_arenas();
        this
    }
    fn init_arenas(&mut self) {
        /*    for i in 0..SIZE_CLASSES.len() {
            self.arenas[i] = Box::into_raw(Box::new(SmallArena::new(SIZE_CLASSES[i])));
        }*/
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn test_and_set_marked(cell: *mut Header) -> bool {
        unsafe {
            if (*cell).tag() == GC_UNMARKED {
                (*cell).set_tag(GC_MARKED);
                true
            } else {
                false
            }
        }
    }

    pub fn add_constraint(&mut self, x: impl MarkingConstraint + 'static) {
        self.constraints.push(Box::new(x));
    }
    fn add_core_constraints(&mut self) {
        /* // we do not want to mark stack when running MIRI.
        #[cfg(not(miri))]
        self.add_constraint(SimpleMarkingConstraint::new(
            "Conservative Roots",
            |marking| {
                let origin = marking.gc.stack_bounds.origin;
                marking.add_conservative_roots(origin, marking.gc.sp as _);
            },
        ));*/
    }

    unsafe fn gc_internal(&mut self, dummy: *const usize) {
        if self.ndefers > 0 {
            return;
        }

        let mut task = Marking {
            gc: self,
            bytes_visited: 0,
            worklist: VecDeque::with_capacity(8),

            file: None,
        };

        task.run();

        let visited = task.bytes_visited;
        drop(task);
        let mut prev: *mut Header = null_mut();
        let mut cur = self.list;
        self.allocated = 0;
        while !cur.is_null() {
            let sz = (*prev).get_dyn().compute_size() + core::mem::size_of::<Header>();
            if (*cur).tag() == GC_MARKED {
                prev = cur;
                cur = (*cur).next;
                (*prev).set_tag(GC_UNMARKED);
                self.allocated += sz;
            } else {
                let unreached = cur;
                cur = (*cur).next;
                if !prev.is_null() {
                    (*prev).next = cur;
                } else {
                    self.list = cur;
                }
                let layout = Layout::from_size_align_unchecked(sz, 16);
                std::ptr::drop_in_place((*unreached).get_dyn());
                heap().free(unreached.cast(), layout);
            }
        }
        //self.allocated = visited;
        if self.allocated >= self.max_heap_size {
            self.max_heap_size = (self.allocated as f64 * 1.7) as usize;
        }
        /*for arena in self.arenas.iter().copied() {
            unsafe {
                (*arena).sweep();
            }
        }

        self.precise_allocations.retain(|alloc| {
            let cell = (**alloc).cell();
            if (*cell).tag() == GC_WHITE {
                (**alloc).destroy();
                false
            } else {
                (*cell).set_tag(GC_WHITE);
                true
            }
        });
        self.precise_allocations.sort_unstable();*/
        self.allocated = visited;
        self.max_heap_size = (visited as f64 * 1.7) as usize;
    }

    pub fn gc(&mut self) {
        let x = 0;

        unsafe {
            self.gc_internal(&x);
        }
    }

    pub fn collect_if_necessary(&mut self) {
        if self.allocated <= self.max_heap_size {
            return;
        }
        self.gc();
    }
    pub fn defer_gc(&mut self) {
        self.ndefers += 1;
    }
    pub fn undefer_gc(&mut self) {
        self.ndefers -= 1;
    }
    #[inline(never)]
    unsafe fn alloc_slow(&mut self, size: usize) -> Address {
        /*assert!(size > 4080);
        let ix = self.precise_allocations.len();
        let precise = PreciseAllocation::try_create(self, size, ix as _);
        self.precise_allocations.push(precise);
        Address::from_ptr((*precise).cell())*/
        unreachable!()
    }

    /// Allocate `size` bytes in GC heap.
    ///
    /// # Safety
    ///
    /// This function is unsafe since it returns partially initialized data.
    /// Only first 8 bytes is initialized with GC object header.
    ///
    ///
    #[inline]
    pub unsafe fn allocate_raw(&mut self, size: usize) -> Address {
        self.collect_if_necessary();
        self.allocated += size;
        /*if size > 4080 {
            self.alloc_slow(size)
        } else {
            let arena = self.arenas[size_class_index_for(size).unwrap()];
            (*arena).allocate(self)
        }*/
        Address::from_ptr(heap().alloc(Layout::from_size_align_unchecked(size, 16)))
    }
    pub fn heap_usage(&self) -> usize {
        self.allocated
    }

    pub fn alloc<T: Cell>(&mut self, value: T) -> Gc<T> {
        unsafe {
            fn allocation_size<T: Cell>(val: &T) -> usize {
                /// Align address upwards.
                ///
                /// Returns the smallest x with alignment `align` so that x >= addr.
                /// The alignment must be a power of 2.
                pub fn align_up(addr: u64, align: u64) -> u64 {
                    assert!(align.is_power_of_two(), "`align` must be a power of two");
                    let align_mask = align - 1;
                    if addr & align_mask == 0 {
                        addr // already aligned
                    } else {
                        (addr | align_mask) + 1
                    }
                }
                align_up(val.compute_size() as u64 + size_of::<Header>() as u64, 16) as usize
                // round_up_to_multiple_of(16, val.compute_size() + size_of::<Header>())
            }
            let size = allocation_size(&value);
            let memory = self.allocate_raw(size).to_mut_ptr::<Header>();
            assert!(!memory.is_null());

            memory.write(Header::new(self, null_mut(), object_ty_of(&value)));
            (*memory).set_tag(GC_UNMARKED);
            let sz = value.compute_size();
            (*memory).data_start().to_mut_ptr::<T>().write(value);
            /*std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                (*memory).data_start().to_mut_ptr::<u8>(),
                sz,
            );*/
            //std::mem::forget(value);
            #[cfg(feature = "valgrind-gc")]
            {
                println!(
                    "Alloc {:p} ({}): {}",
                    memory,
                    std::any::type_name::<T>(),
                    std::backtrace::Backtrace::capture()
                );
            }
            (*memory).next = self.list;
            self.list = memory;
            Gc {
                cell: NonZeroCompressed::new_unchecked(compress_ptr(memory as _)),
                marker: Default::default(),
            }
        }
    }
}
pub struct Marking<'a> {
    pub gc: &'a mut Heap,
    pub worklist: VecDeque<*mut Header>,
    pub bytes_visited: usize,
    #[allow(dead_code)]
    file: Option<&'a mut std::fs::File>,
}

impl<'a> Marking<'a> {
    pub fn run(&mut self) {
        self.process_constraints();
        self.process_roots();
        self.process_worklist();
    }
    fn process_constraints(&mut self) {
        unsafe {
            let mut constraints = vec![];
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
            for c in constraints.iter_mut() {
                c.execute(self);
            }
            std::mem::swap(&mut constraints, &mut self.gc.constraints);
        }
    }
    fn process_roots(&mut self) {
        unsafe {
            /*let mut head = self.gc.scopes;
            while !head.is_null() {
                let scope = &mut *head;
                scope.roots.retain(|item| {
                    /*if item.is_null() {
                        false
                    } else {
                        self.mark(*item);
                        true
                    }*/
                    match item {
                        Some(ptr) => {
                            (*ptr.as_ptr()).trace(self);
                            true
                        }
                        None => false,
                    }
                });
                head = (*head).next;
            }*/
            let this = self as *mut Self;
            for handle in self.gc.handles.iter().copied() {
                (*handle).trace(&mut *this);
            }
        }
    }
    fn process_worklist(&mut self) {
        while let Some(item) = self.worklist.pop_front() {
            unsafe {
                self.visit_value(item);
            }
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn mark(&mut self, val: *mut Header) {
        unsafe {
            if Heap::test_and_set_marked(val) {
                let obj = val;
                //println!("{}", obj.get_dyn().get_typename());
                self.bytes_visited += round_up_to_multiple_of(
                    16,
                    (*obj).get_dyn().compute_size() + core::mem::size_of::<Header>(),
                );
                self.worklist.push_back(obj);
            }
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn visit_value(&mut self, val: *mut Header) {
        unsafe {
            (*val).get_dyn().trace(self);
        }
    }
    /*pub fn add_conservative_roots(&mut self, from: *mut u8, to: *mut u8) {
        self.cons.scan.push((from, to));
    }

    #[allow(clippy::mutable_key_type)]
    unsafe fn find_gc_object_pointer_for_marking(
        &mut self,
        ptr: *mut u8,
        mut f: impl FnMut(&mut Self, *mut Header),
    ) {
        if !self.gc.precise_allocations.is_empty() {
            if (**self.gc.precise_allocations.first().unwrap()).above_lower_bound(ptr.cast())
                && (**self.gc.precise_allocations.last().unwrap()).below_upper_bound(ptr.cast())
            {
                let result = self
                    .gc
                    .precise_allocations
                    .binary_search(&PreciseAllocation::from_cell(ptr.cast()));
                match result {
                    Ok(ix) => {
                        if (*self.gc.precise_allocations[ix]).has_valid_cell {
                            f(self, ptr.cast());
                        }
                    }
                    _ => (),
                }
            }
        }
        let filter = self.gc.block_set.filter;
        let set = &self.gc.block_set.set;
        let candidate = HeapBlock::from_cell(ptr.cast());
        if filter.rule_out(candidate as _) {
            return;
        }

        if !set.contains(&candidate) {
            return;
        }

        let mut try_ptr = |ptr| {
            let is_live = (*candidate).cell_from_possible_pointer(Address::from_ptr(ptr));
            if !is_live.is_null() && !(*is_live).is_zapped() {
                f(self, ptr as *mut _);
                true
            } else {
                false
            }
        };

        if try_ptr(ptr) {
            return;
        }
    }*/
}

impl<'a> Tracer for Marking<'a> {
    fn trace(&mut self, hdr: *mut Header) {
        self.mark(hdr);
    }
}

#[cfg(not(any(target_os = "windows", feature = "valgrind-gc")))]
impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            let mut object = self.list;
            while !object.is_null() {
                let obj = object;
                object = (*obj).next;
                let sz = (*obj).get_dyn().compute_size() + size_of::<Header>();
                std::ptr::drop_in_place((*obj).get_dyn());
                heap().free(obj.cast(), Layout::from_size_align_unchecked(sz, 16));
            }
            self.constraints.clear();
        }
    }
}

impl AsMut<Heap> for &mut Heap {
    fn as_mut(&mut self) -> &mut Heap {
        self
    }
}
