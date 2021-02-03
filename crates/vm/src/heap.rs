//! #  Starlight heap implementation
//! Starlight uses conservative version of immix garbage collection algorithm combined with
//! opportunistic evacuation.
//!
//! ## Features
//! - No need for special handling of heap references on stack. GC is smart enough to detect them
//! on stack automatically.
//! - Fast allocation times
//!     
//!
//!     Immix allows for bump allocating always but there is one exception: Large objects (>8KB). Large objects
//!     is allocated on system heap using `libc::malloc` and `libc::free`.
//! - Fast collection times
//!     
//!
//!     Immix does not mark objects but marks blocks and lines in blocks. This allows us to recycle only lines
//!     or entire blocks rather than working per object which greatly speed ups GC cycles.
//! - Opportunistic evacuation
//!
//!      
//!     When heap is fragmented evacuation might be enabled. This means that before doing GC some blocks will be marked
//!     as blocks that need evacuation and during tracing objects from these blocks will be evacuated. Note that pinned objects
//!     is not evacuated (pinned = object is found conservatively on stack).
//!

use crate::gc::heap_cell::{HeapCell, HeapCellU, HeapObject};
use allocator::ImmixSpace;
use collection::Collector;
use constants::LARGE_OBJECT;
use large_object_space::{LargeObjectSpace, PreciseAllocation};
use std::mem::transmute;
use tagged_pointer::TaggedPointer;
use util::address::Address;
use util::*;
use wtf_rs::{stack_bounds::StackBounds, TraitObject};

use crate::runtime::{ref_ptr::Ref, vm::JsVirtualMachine};
#[macro_use]
pub mod util;
pub mod allocator;
pub mod block;
pub mod block_allocator;
pub mod collection;
pub mod constants;
pub mod header;
pub mod large_object_space;
pub mod snapshot;
pub mod space_bitmap;
pub mod trace;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CollectionType {
    ImmixEvacCollection,
    ImmixCollection,
}

/// Heap implementation. Look module level documentation for more docs.
pub struct Heap {
    pub(crate) los: LargeObjectSpace,
    pub(crate) immix: *mut ImmixSpace,
    /// The total allocated bytes on the heap.
    pub(crate) allocated: usize,
    /// The maximum number of bytes which will trigger a GC collection.
    /// If collection does not lower the allocated size below the threshold then the threshold
    /// will be raised.
    threshold: usize,
    pub(crate) current_live_mark: bool,
    collector: Collector,
    pub(crate) vm: Ref<JsVirtualMachine>,
}

#[inline(never)]
fn get_stack_pointer() -> usize {
    let x = 0x400usize;
    &x as *const usize as usize
}

impl Heap {
    pub(crate) fn new(vm: Ref<JsVirtualMachine>, size: usize, threshold: usize) -> Self {
        Self {
            los: LargeObjectSpace::new(vm),
            allocated: 0,
            threshold,
            vm,
            immix: ImmixSpace::new(vm, size),
            current_live_mark: false,
            collector: Collector::new(),
        }
    }
    /// Force garbage collection. If `evac` is set to true then GC has higher chances of doing evacuation cycle.
    pub fn gc(&mut self, evac: bool) {
        unsafe {
            self.collect_internal(evac, false);
        }
    }

    unsafe fn collect_internal(&mut self, evacuation: bool, emergency: bool) {
        log_if!(
            self.vm.options.verbose_gc,
            "- Initiating GC cycle after {} bytes allocated",
            self.allocated
        );
        let mut precise_roots = Vec::new();
        // Stage #1: Collect all precisely known objects.
        for (_, sym) in self.vm.symbols.iter_mut() {
            precise_roots.push(transmute(sym));
        }
        let mut roots: Vec<*mut HeapCell> = Vec::new();
        let all_blocks = (*self.immix).get_all_blocks();
        {
            // Stage #2: Collect thread stack roots.
            let bounds = StackBounds::current_thread_stack_bounds();
            self.collect_roots(
                bounds.origin as *mut *mut u8,
                get_stack_pointer() as *mut *mut u8,
                &mut roots,
            );
        }
        self.collector.extend_all_blocks(all_blocks);
        // Stage #3: Prepare collection and choose collection type
        let collection_type = self.collector.prepare_collection(
            evacuation,
            true,
            (*(*self.immix).block_allocator).available_blocks(),
            (*self.immix).evac_headroom(),
            (*(*self.immix).block_allocator).total_blocks(),
            emergency,
        );
        // Stage #4: Mark all reachable objects.
        let visited = self.collector.collect(
            self.vm.options.verbose_gc,
            &(*self.immix).bitmap,
            &collection_type,
            &roots,
            &precise_roots,
            &mut *self.immix,
            &mut self.los,
            !self.current_live_mark,
        );
        // Stage #5: Unpin all pinned objects.
        for root in roots.iter() {
            (&mut **root).unpin()
        }
        // Stage #6: Swap live mark so next GC cycle will know how to mark objects.
        self.current_live_mark = !self.current_live_mark;
        (*self.immix).set_current_live_mark(self.current_live_mark);
        self.los.current_live_mark = self.current_live_mark;
        //let prev = self.allocated;
        // Increase GC threshold if needed.
        self.allocated = visited;
        if visited >= self.threshold {
            self.threshold = (visited as f64 * 1.75) as usize;
        }

        // TODO: Add a way to shrink threshold automatically.

        log_if!(
            self.vm.options.verbose_gc,
            "- GC end with {:.3}KiB heap and {:.4}KiB threshold",
            self.allocated as f32 / 1024.0,
            self.threshold as f32 / 1024f32
        )
    }
    /// Scan all pointers in `from`..`to` range and if any heap handle is found it is pushed to `into` vector
    /// for further scan in mark cycle.
    pub(crate) unsafe fn collect_roots(
        &mut self,
        from: *mut *mut u8,
        to: *mut *mut u8,
        into: &mut Vec<*mut HeapCell>,
    ) {
        // we want to align stack pointers to 16 bytes.
        let mut scan = align_usize(from as usize, 16) as *mut *mut u8;
        let mut end = align_usize(to as usize, 16) as *mut *mut u8;
        if scan.is_null() || end.is_null() {
            return;
        }
        // if stack grows in other way we just swap scan and end.
        if scan > end {
            core::mem::swap(&mut scan, &mut end);
        }

        while scan < end {
            let ptr = *scan;
            if ptr.is_null() {
                scan = scan.offset(1);
                continue;
            }
            // If object is large object (bit 8 is set) and binary search through all
            // large objects finds `ptr` we push it to mark stack.
            if PreciseAllocation::is_precise(ptr.cast())
                && self.los.contains(Address::from_ptr(ptr))
            // ^ binary search
            {
                (&mut *ptr.cast::<HeapCell>()).pin();
                into.push(ptr.cast::<HeapCell>());
                scan = scan.offset(1);
                continue;
            }
            // first try: check if `ptr` is allocated in immix space (read ImmixSpace::filter docs for more info
            // on how it detects pointers).
            if let Some(ptr) = (*self.immix).filter(Address::from_ptr(ptr)) {
                let ptr = ptr.to_mut_ptr::<u8>();

                (&mut *ptr.cast::<HeapCell>()).pin();

                into.push(ptr.cast());
            }
            // second try: move pointer back by 8 bytes maybe it is reference to data in handle?
            let ptr = ptr.sub(8);
            if let Some(ptr) = (*self.immix).filter(Address::from_ptr(ptr)) {
                let ptr = ptr.to_mut_ptr::<u8>();

                (&mut *ptr.cast::<HeapCell>()).pin();

                into.push(ptr.cast());
            }
            scan = scan.offset(1);
        }
    }
    /// Allocate `value` on the heap with `size` bytes.
    pub(crate) unsafe fn allocate<T: HeapObject>(&mut self, value: T, size: usize) -> Address {
        if self.allocated >= self.threshold {
            // Achtung! We reached GC threshold and GC should happen now.
            self.collect_internal(false, false);
        }
        let info = &value as &dyn HeapObject;
        let trait_object = transmute::<_, TraitObject>(info);
        let size = align_usize(size + 8, 16);
        let ptr = if size >= LARGE_OBJECT {
            // bad case :( Large objects is not a good thing.
            self.los.alloc(size)
        } else {
            // good case. If objects fits into immix space it is just bump allocated.
            let mut addr = (*self.immix).allocate(size, value.needs_destruction());
            if addr.is_null() {
                // out of memory? No problem lets try to do emergency GC cycle.
                self.collect_internal(true, true);
                addr = (*self.immix).allocate(size, value.needs_destruction());
                if addr.is_null() {
                    // still no memory? Panic.
                    panic!("Out of memory");
                }
            }

            Address::from_ptr(addr)
        };
        self.allocated += size;
        let raw = ptr.to_mut_ptr::<HeapCell>();
        (*raw).data().to_mut_ptr::<T>().write(value);
        *raw = HeapCell {
            u: HeapCellU {
                tagged: TaggedPointer::new(trait_object.vtable.cast()),
            },
        };
        (*raw).mark(self.current_live_mark);

        ptr
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.immix);
        }
    }
}
