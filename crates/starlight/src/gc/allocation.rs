use std::{
    fmt::Debug,
    mem::size_of,
    ptr::{drop_in_place, null_mut},
};

use wtf_rs::round_up;

use crate::{
    gc::{block::Block, constants::BLOCK_SIZE},
    gc::{
        cell::{GcPointerBase, DEFINETELY_WHITE, POSSIBLY_BLACK},
        Address,
    },
};

use super::{
    block::FreeList,
    block_allocator::BlockAllocator,
    large_object_space::{LargeObjectSpace, PreciseAllocation},
    space_bitmap::SpaceBitmap,
};

/// Sizes up to this amount get a size class for each size step.
const PRECISE_CUTOFF: usize = 80;
const SIZE_STEP: usize = 16;
const LARGE_CUTOFF: usize = ((BLOCK_SIZE - size_of::<Block>()) / 2) & !(SIZE_STEP - 1);
const BLOCK_PAYLOAD: usize = BLOCK_SIZE - size_of::<Block>();
fn generate_size_classes(dump_size_classes: bool, sz_class_progression: f64) -> Vec<usize> {
    let mut result = vec![];
    let mut add = |result: &mut Vec<usize>, size_class| {
        logln_if!(dump_size_classes, "Adding size class: {}", size_class);
        if result.is_empty() {
            assert_eq!(size_class, 16);
        }
        result.push(size_class);
    };

    let mut size = 16;
    while size < PRECISE_CUTOFF {
        add(&mut result, size);
        size += SIZE_STEP;
    }
    logln_if!(
        dump_size_classes,
        "       Block payload size: {}",
        BLOCK_SIZE - offsetof!(Block.data_start)
    );

    for i in 0.. {
        let approximate_size = PRECISE_CUTOFF as f64 * sz_class_progression.powi(i);
        logln_if!(
            dump_size_classes,
            "     Next size class as a double: {}",
            approximate_size
        );
        let approximate_size_in_bytes = approximate_size as usize;
        logln_if!(
            dump_size_classes,
            "     Next size class as bytes: {}",
            approximate_size_in_bytes
        );
        assert!(approximate_size_in_bytes >= PRECISE_CUTOFF);

        if approximate_size_in_bytes >= LARGE_CUTOFF {
            break;
        }
        let size_class = round_up(approximate_size_in_bytes, SIZE_STEP);
        logln_if!(dump_size_classes, "     Size class: {}", size_class);

        let cells_per_block = BLOCK_PAYLOAD / size_class;
        let possibly_better_size_class = (BLOCK_PAYLOAD / cells_per_block) & !(SIZE_STEP - 1);
        logln_if!(
            dump_size_classes,
            "     Possibly better size class: {}",
            possibly_better_size_class
        );
        let original_wastage = BLOCK_PAYLOAD - cells_per_block * size_class;
        let new_wastage = (possibly_better_size_class - size_class) * cells_per_block;
        logln_if!(
            dump_size_classes,
            "    Original wastage: {}, new wastage: {}",
            original_wastage,
            new_wastage
        );

        let better_size_class = if new_wastage > original_wastage {
            size_class
        } else {
            possibly_better_size_class
        };
        logln_if!(
            dump_size_classes,
            "    Choosing size class: {}",
            better_size_class
        );
        if Some(better_size_class) == result.last().copied() {
            // when size class step is too small
            continue;
        }

        if better_size_class > LARGE_CUTOFF {
            break;
        }
        add(&mut result, better_size_class);
    }
    // Manually inject size classes for objects we know will be allocated in high volume.

    add(&mut result, 256);
    //add(&mut result, size_of::<JsObject>());
    result.sort_unstable();
    result.dedup();
    result.shrink_to_fit();
    logln_if!(dump_size_classes, "Heap size class dump: {:?}", result);

    result
}

const NUM_SIZE_CLASSES: usize = LARGE_CUTOFF / SIZE_STEP + 1;
fn build_size_class_table(
    dump: bool,
    progression: f64,
    table: &mut [usize],
    cons: impl Fn(usize) -> usize,
    default_cons: impl Fn(usize) -> usize,
) {
    let mut next_index = 0;
    for sz in generate_size_classes(dump, progression) {
        let entry = cons(sz);
        let index = size_class_to_index(sz);
        for i in next_index..=index {
            table[i] = entry;
        }
        next_index = index + 1;
    }
    for i in next_index..NUM_SIZE_CLASSES {
        table[i] = default_cons(index_to_size_class(i));
    }
}
fn initialize_size_class_for_step_size(dump: bool, progression: f64, table: &mut [usize]) {
    build_size_class_table(dump, progression, table, |sz| sz, |sz| sz);
}

const fn size_class_to_index(size: usize) -> usize {
    (size + SIZE_STEP - 1) / SIZE_STEP
}

fn index_to_size_class(index: usize) -> usize {
    let result = index * SIZE_STEP;
    debug_assert_eq!(size_class_to_index(result), index);
    result
}

pub struct Space {
    precise_allocations: LargeObjectSpace,
    allocator_for_size_class: Box<[Option<LocalAllocator>]>,
    size_class_for_size_step: [usize; NUM_SIZE_CLASSES],
    block_allocator: *mut BlockAllocator,
    allocators: *mut LocalAllocator,
    live_bitmap: SpaceBitmap<16>,
    mark_bitmap: SpaceBitmap<16>,
}

impl Drop for Space {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.block_allocator);
        }
    }
}

impl Space {
    unsafe fn allocator_for_slow<'a>(&'a mut self, size: usize) -> Option<*mut LocalAllocator> {
        let index = size_class_to_index(size);
        let size_class = self.size_class_for_size_step[index];
        if size_class == 0 {
            return None;
        }

        if let Some(ref mut allocator) = self.allocator_for_size_class[index] {
            return Some(allocator);
        }

        let mut alloc = LocalAllocator::new(size_class, self.block_allocator);
        alloc.next = self.allocators;
        self.allocator_for_size_class[index] = Some(alloc);
        self.allocators = self.allocator_for_size_class[index].as_mut().unwrap();
        self.allocator_for_size_class[index]
            .as_mut()
            .map(|x| x as *mut _)
    }

    pub fn for_each_cell(&self, cb: &mut dyn FnMut(*mut GcPointerBase, usize) -> bool) {
        unsafe {
            let mut alloc = self.allocators;
            while !alloc.is_null() {
                let mut block = (*alloc).current;
                while !block.is_null() {
                    (*block).walk(|ptr| {
                        if self.live_bitmap.test(ptr as _) {
                            cb(ptr.cast(), (*block).cell_size.get() as _);
                        }
                    });
                    block = (*block).next;
                }

                block = (*alloc).unavail;
                while !block.is_null() {
                    (*block).walk(|ptr| {
                        if self.live_bitmap.test(ptr as _) {
                            cb(ptr.cast(), (*block).cell_size.get() as _);
                        }
                    });
                    block = (*block).next;
                }

                alloc = (*alloc).next;
            }

            self.precise_allocations
                .allocations
                .iter()
                .for_each(|alloc| {
                    cb((**alloc).cell().cast(), (**alloc).cell_size());
                });
        }
    }

    pub fn sweep(&mut self) -> usize {
        let mut allocated = self.precise_allocations.sweep();
        unsafe {
            let mut alloc = self.allocators;

            while !alloc.is_null() {
                let next = (*alloc).next;
                (*alloc).sweep(&mut allocated, &self.live_bitmap, &self.mark_bitmap);
                alloc = next;
            }
        }
        allocated
    }
    #[inline]
    pub fn allocate(&mut self, size: usize, threshold: &mut usize) -> *mut u8 {
        

        if size <= LARGE_CUTOFF {
            let p = self.allocate_small(size, threshold);
            debug_assert!(!self.live_bitmap.test(p as _));
            self.live_bitmap.set(p as _);
            debug_assert!(self.live_bitmap.test(p as _));
            p
        } else {
            self.precise_allocations.alloc(size, threshold).to_mut_ptr()
        }
    }
    fn allocate_small(&mut self, size: usize, threshold: &mut usize) -> *mut u8 {
        unsafe {
            *threshold += self.size_class_for_size_step[size_class_to_index(size)];
            let result = self.allocator_for_size_class[size_class_to_index(size)].as_mut();
            if result.is_some() {
                return result.unwrap().allocate();
            }
            (*self
                .allocator_for_slow(size)
                .unwrap_or_else(|| unreachable!()))
            .allocate()
        }
    }

    pub fn new(size: usize, dump: bool, progression: f64) -> Self {
        let mut alloc = Box::into_raw(Box::new(BlockAllocator::new(size)));
        unsafe {
            let mut bitmap = SpaceBitmap::<16>::create("live-bitmap", (*alloc).mmap.start(), size);
            let mark_bitmap = SpaceBitmap::<16>::create("mark-bitmap", (*alloc).mmap.start(), size);
            let mut this = Self {
                allocator_for_size_class: vec![None; NUM_SIZE_CLASSES].into_boxed_slice(),
                live_bitmap: bitmap,
                block_allocator: alloc,
                allocators: null_mut(),
                mark_bitmap,
                size_class_for_size_step: [0; NUM_SIZE_CLASSES],
                precise_allocations: LargeObjectSpace::new(),
            };
            initialize_size_class_for_step_size(
                dump,
                progression,
                &mut this.size_class_for_size_step,
            );
            this
        }
    }
    pub fn is_heap_pointer(&self, ptr: *const u8) -> bool {
        unsafe {
            if self.live_bitmap.has_address(ptr) {
                return self.live_bitmap.test(ptr as _);
            }
            self.precise_allocations.contains(Address::from_ptr(ptr))
        }
    }

    pub fn mark(&self, ptr: *const GcPointerBase) -> bool {
        if self.live_bitmap.has_address(ptr.cast()) {
            return self.mark_bitmap.set(ptr.cast());
        } else {
            unsafe {
                let prec = PreciseAllocation::from_cell(ptr as _);
                (*prec).test_and_set_marked()
            }
        }
    }
}
#[derive(Clone, Copy)]
pub struct LocalAllocator {
    cell_size: usize,
    next: *mut LocalAllocator,
    current: *mut Block,
    unavail: *mut Block,
    allocator: *mut BlockAllocator,
}
impl LocalAllocator {
    unsafe fn sweep(
        &mut self,
        allocated: &mut usize,
        live_bitmap: &SpaceBitmap<16>,
        mark_bitmap: &SpaceBitmap<16>,
    ) {
        let mut available = null_mut();
        let mut unavailable = null_mut();
        let mut cursor = self.current;

        while !cursor.is_null() {
            let next = (*cursor).next;
            let mut freelist = FreeList::new();
            let mut has_free = false;
            let mut fully_free = true;
            (*cursor).walk(|cell| {
                if live_bitmap.test(cell as _) {
                    let cell = cell.cast::<GcPointerBase>();
                    let state = (*cell).state();
                    debug_assert!(state == DEFINETELY_WHITE || state == POSSIBLY_BLACK);
                    if mark_bitmap.clear(cell as _) {
                        fully_free = false;

                        (*cell).force_set_state(DEFINETELY_WHITE);
                        *allocated += (*cursor).cell_size.get() as usize;
                    } else {
                        debug_assert!(!mark_bitmap.test(cell as _));
                        live_bitmap.clear(cell as _);
                        has_free = true;
                        drop_in_place((*cell).get_dyn());
                        freelist.add(cell.cast());
                    }
                } else {
                    debug_assert!(!mark_bitmap.test(cell as _));
                    has_free = true;
                    debug_assert!(!live_bitmap.test(cell as _));
                    freelist.add(cell.cast());
                }
            });
            (*cursor).freelist = freelist;
            if (*cursor).freelist.next.is_null() {
                // if block is full we do not want to try to allocate from it.
                (*cursor).next = unavailable;
                unavailable = cursor;
            } else if fully_free {
                assert!(!(*cursor).freelist.next.is_null());
                (*self.allocator).return_block(cursor);
            } else {
                (*cursor).next = available;
                available = cursor;
            }
            cursor = next;
        }
        cursor = self.unavail;
        while !cursor.is_null() {
            let next = (*cursor).next;
            let mut freelist = FreeList::new();
            let mut has_free = false;
            let mut fully_free = true;
            (*cursor).walk(|cell| {
                if live_bitmap.test(cell as _) {
                    let cell = cell.cast::<GcPointerBase>();
                    let state = (*cell).state();
                    debug_assert!(state == DEFINETELY_WHITE || state == POSSIBLY_BLACK);
                    if mark_bitmap.clear(cell as _) {
                        fully_free = false;

                        (*cell).force_set_state(DEFINETELY_WHITE);
                        *allocated += (*cursor).cell_size.get() as usize;
                    } else {
                        debug_assert!(!mark_bitmap.test(cell as _));
                        live_bitmap.clear(cell as _);
                        has_free = true;
                        drop_in_place((*cell).get_dyn());
                        freelist.add(cell.cast());
                    }
                } else {
                    debug_assert!(!mark_bitmap.test(cell as _));
                    has_free = true;
                    debug_assert!(!live_bitmap.test(cell as _));
                    freelist.add(cell.cast());
                }
            });
            (*cursor).freelist = freelist;
            if (*cursor).freelist.next.is_null() {
                // if block is full we do not want to try to allocate from it.
                (*cursor).next = unavailable;
                unavailable = cursor;
            } else if fully_free {
                assert!(!(*cursor).freelist.next.is_null());
                (*self.allocator).return_block(cursor);
            } else {
                (*cursor).next = available;
                available = cursor;
            }
            cursor = next;
        }
        self.current = available;
        self.unavail = unavailable;
    }
    pub fn new(cell_size: usize, allocator: *mut BlockAllocator) -> Self {
        Self {
            cell_size,
            allocator,
            current: null_mut(),
            unavail: null_mut(),
            next: null_mut(),
        }
    }
    pub fn allocate(&mut self) -> *mut u8 {
        unsafe {
            let mut block = self.current;
            if block.is_null() {
                self.current = (*self.allocator)
                    .get_block()
                    .unwrap_or_else(|| panic!("{:?}", GCOOM(self.cell_size)));
                block = self.current;
                (*block).init(self.cell_size as _);
            }
            let mut ptr = (*block).allocate();
            if ptr.is_null() {
                let next = (*block).next;
                (*block).next = self.unavail;
                self.unavail = block;
                block = next;
                if !next.is_null() {
                    self.current = next;
                } else {
                    self.current = (*self.allocator)
                        .get_block()
                        .unwrap_or_else(|| panic!("{:?}", GCOOM(self.cell_size)));
                    block = self.current;
                    (*block).init(self.cell_size as _);
                }
                ptr = (*block).allocate();
            }

            ptr
        }
    }
}

pub struct GCOOM(usize);

impl Debug for GCOOM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GC Heap Out of memory satisfying allocation of size: {}.\n Help: Try to increase GC heap size",
            self.0
        )
    }
}
