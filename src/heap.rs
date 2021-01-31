use allocator::ImmixSpace;
use collection::Collector;
use constants::LARGE_OBJECT;
use header::*;
use large_object_space::{LargeObjectSpace, PreciseAllocation};
use std::mem::transmute;
use util::address::Address;
use util::*;
use wtf_rs::stack_bounds::StackBounds;

use crate::runtime::{
    ref_ptr::Ref,
    type_info::{Type, TypeInfo},
    vm::JSVirtualMachine,
};
#[macro_use]
pub mod util;
pub mod allocator;
pub mod block;
pub mod block_allocator;
pub mod collection;
pub mod constants;
pub mod header;
pub mod large_object_space;
pub mod space_bitmap;
pub mod trace;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CollectionType {
    ImmixEvacCollection,
    ImmixCollection,
}

pub struct Heap {
    los: LargeObjectSpace,
    immix: *mut ImmixSpace,
    allocated: usize,
    threshold: usize,
    current_live_mark: bool,
    collector: Collector,
    pub(crate) vm: Ref<JSVirtualMachine>,
}

#[inline(never)]
fn get_stack_pointer() -> usize {
    let x = 0x400usize;
    &x as *const usize as usize
}

impl Heap {
    pub(crate) fn new(vm: Ref<JSVirtualMachine>, size: usize, threshold: usize) -> Self {
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
        for (_, sym) in self.vm.symbols.iter_mut() {
            precise_roots.push(transmute(sym));
        }
        let mut roots: Vec<*mut Header> = Vec::new();
        let all_blocks = (*self.immix).get_all_blocks();
        {
            let bounds = StackBounds::current_thread_stack_bounds();
            self.collect_roots(
                bounds.origin as *mut *mut u8,
                get_stack_pointer() as *mut *mut u8,
                &mut roots,
            );
        }
        self.collector.extend_all_blocks(all_blocks);
        let collection_type = self.collector.prepare_collection(
            evacuation,
            true,
            (*(*self.immix).block_allocator).available_blocks(),
            (*self.immix).evac_headroom(),
            (*(*self.immix).block_allocator).total_blocks(),
            emergency,
        );
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
        for root in roots.iter() {
            {
                (&mut **root).unpin()
            };
        }
        self.current_live_mark = !self.current_live_mark;
        (*self.immix).set_current_live_mark(self.current_live_mark);
        self.los.current_live_mark = self.current_live_mark;
        //let prev = self.allocated;
        self.allocated = visited;
        if visited >= self.threshold {
            self.threshold = (visited as f64 * 1.75) as usize;
        }

        log_if!(
            self.vm.options.verbose_gc,
            "- GC end with {:.3}KiB heap and {:.4}KiB threshold",
            self.allocated as f32 / 1024.0,
            self.threshold as f32 / 1024f32
        )
    }
    unsafe fn collect_roots(
        &mut self,
        from: *mut *mut u8,
        to: *mut *mut u8,
        into: &mut Vec<*mut Header>,
    ) {
        let mut scan = align_usize(from as usize, 16) as *mut *mut u8;
        let mut end = align_usize(to as usize, 16) as *mut *mut u8;
        if scan.is_null() || end.is_null() {
            return;
        }
        if scan > end {
            core::mem::swap(&mut scan, &mut end);
        }

        while scan < end {
            let ptr = *scan;
            if ptr.is_null() {
                scan = scan.offset(1);
                continue;
            }

            if PreciseAllocation::is_precise(ptr.cast())
                && self.los.contains(Address::from_ptr(ptr))
            {
                (&mut *ptr.cast::<Header>()).pin();
                into.push(ptr.cast::<Header>());
                scan = scan.offset(1);
                continue;
            }
            /*pub fn align_down(addr: usize, align: usize) -> usize {
                /*if !align.is_power_of_two() {
                    panic!("align should be power of two");
                }*/
                addr & !(align - 1)
            }*/
            //let ptr = align_down(ptr as usize, 16) as *mut u8;
            if let Some(ptr) = (*self.immix).filter(Address::from_ptr(ptr)) {
                let ptr = ptr.to_mut_ptr::<u8>();

                (&mut *ptr.cast::<Header>()).pin();

                into.push(ptr.cast());
            }
            let ptr = ptr.sub(8);
            if let Some(ptr) = (*self.immix).filter(Address::from_ptr(ptr)) {
                let ptr = ptr.to_mut_ptr::<u8>();

                (&mut *ptr.cast::<Header>()).pin();

                into.push(ptr.cast());
            }
            scan = scan.offset(1);
        }
    }

    pub(crate) unsafe fn allocate<T: Type>(
        &mut self,
        value: T,
        size: usize,
        ty_info: &'static TypeInfo,
    ) -> Address {
        if self.allocated >= self.threshold {
            self.collect_internal(false, true);
        }
        let size = align_usize(size, 16);
        let ptr = if size >= LARGE_OBJECT {
            self.los.alloc(size, ty_info)
        } else {
            let mut addr = (*self.immix).allocate(size, ty_info.needs_destruction);
            if addr.is_null() {
                self.collect_internal(true, true);
                addr = (*self.immix).allocate(size, ty_info.needs_destruction);
                if addr.is_null() {
                    panic!("Out of memory");
                }
            }

            Address::from_ptr(addr)
        };
        self.allocated += size;
        let raw = ptr.to_mut_ptr::<Header>();
        raw.cast::<T>().write(value);
        *raw = Header::new(ty_info);
        (*raw).mark(self.current_live_mark);

        ptr
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.immix);
        }
    }
}
