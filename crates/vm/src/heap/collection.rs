use crate::gc::heap_cell::HeapCell;

use super::{
    allocator::ImmixSpace,
    block::ImmixBlock,
    constants::*,
    large_object_space::LargeObjectSpace,
    space_bitmap::SpaceBitmap,
    trace::Slot,
    trace::Tracer,
    trace::TracerPtr,
    util::{address::Address, align_usize},
    CollectionType,
};
use std::collections::VecDeque;
use vec_map::VecMap;

const GC_VERBOSE: bool = true;

pub struct ImmixCollector;
pub struct Visitor<'a> {
    immix_space: &'a mut ImmixSpace,
    queue: &'a mut VecDeque<*mut HeapCell>,
    defrag: bool,
    next_live_mark: bool,
}

impl Tracer for Visitor<'_> {
    fn trace(&mut self, slot: Slot) {
        unsafe {
            let mut child = &mut *slot.value();
            if child.is_forwarded() {
                slot.set(child.vtable());
            } else if child.get_mark() != self.next_live_mark {
                if self.defrag && self.immix_space.filter_fast(Address::from_ptr(child)) {
                    if let Some(new_child) = self.immix_space.maybe_evacuate(child) {
                        slot.set(new_child);
                        child = &mut *new_child.to_mut_ptr::<HeapCell>();
                    }
                }
                self.queue.push_back(child);
            }
        }
    }
}

impl ImmixCollector {
    pub fn collect(
        collection_type: &CollectionType,
        roots: &[*mut HeapCell],
        precise_roots: &[*mut *mut HeapCell],
        immix_space: &mut ImmixSpace,
        next_live_mark: bool,
    ) -> usize {
        let mut object_queue: VecDeque<*mut HeapCell> = roots.iter().copied().collect();
        for root in precise_roots.iter() {
            unsafe {
                let root = &mut **root;
                let mut raw = &mut **root;
                if immix_space.filter_fast(Address::from_ptr(raw)) {
                    if raw.is_forwarded() {
                        raw = &mut *(raw.vtable().to_mut_ptr::<HeapCell>());
                    } else if *collection_type == CollectionType::ImmixEvacCollection {
                        if let Some(new_object) = immix_space.maybe_evacuate(raw) {
                            *root = new_object.to_mut_ptr::<HeapCell>();
                            raw.set_forwarded(new_object);
                            raw = &mut *new_object.to_mut_ptr::<HeapCell>();
                        }
                    }
                }
                object_queue.push_back(raw);
            }
        }
        let mut visited = 0;

        while let Some(object) = object_queue.pop_front() {
            unsafe {
                //debug!("Process object {:p} in Immix closure", object);
                let object_addr = Address::from_ptr(object);
                if !(&mut *object).mark(next_live_mark) {
                    if immix_space.filter_fast(object_addr) {
                        let block = ImmixBlock::get_block_ptr(object_addr);
                        immix_space.set_gc_object(object_addr); // Mark object in bitmap
                        (&mut *block).line_object_mark(object_addr); // Mark block line
                    }

                    visited += align_usize((*object).get_dyn().compute_size() + 8, 16);
                    let mut visitor = Visitor {
                        immix_space,
                        next_live_mark,
                        queue: &mut object_queue,
                        defrag: *collection_type == CollectionType::ImmixEvacCollection,
                    };

                    /*visitor_fn(
                        Address::from_ptr(object),
                        TracerPtr {
                            tracer: core::mem::transmute(&mut visitor as &mut dyn Tracer),
                        },
                    );*/
                    (*object).get_dyn().visit_children(&mut visitor);
                }
            }
        }
        // debug!("Completed collection with {} bytes visited", visited);
        visited
    }
}

pub struct Collector {
    all_blocks: Vec<*mut ImmixBlock>,
    mark_histogram: VecMap<usize>,
}
impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
impl Collector {
    pub fn new() -> Self {
        Self {
            all_blocks: Vec::new(),
            mark_histogram: VecMap::with_capacity(NUM_LINES_PER_BLOCK),
        }
    }
    /// Store the given blocks into the buffer for use during the collection.
    pub fn extend_all_blocks(&mut self, blocks: Vec<*mut ImmixBlock>) {
        self.all_blocks.extend(blocks);
    }
    /// Prepare a collection.
    ///
    /// This function decides if a evacuating and/or cycle collecting
    /// collection will be performed. If `evacuation` is set the collectors
    /// will try to evacuate. If `cycle_collect` is set the immix tracing
    /// collector will be used.
    pub fn prepare_collection(
        &mut self,
        evacuation: bool,
        _cycle_collect: bool,
        available_blocks: usize,
        evac_headroom: usize,
        total_blocks: usize,
        emergency: bool,
    ) -> CollectionType {
        if emergency && USE_EVACUATION {
            for block in &mut self.all_blocks {
                unsafe {
                    (**block).evacuation_candidate = true;
                }
            }
            return CollectionType::ImmixEvacCollection;
        }
        let mut perform_evac = evacuation;

        let evac_threshhold = (total_blocks as f64 * EVAC_TRIGGER_THRESHHOLD) as usize;

        let available_evac_blocks = available_blocks + evac_headroom;

        if evacuation || available_evac_blocks < evac_threshhold {
            let hole_threshhold = self.establish_hole_threshhold(evac_headroom);

            perform_evac = USE_EVACUATION && hole_threshhold > 0;
            if perform_evac {
                for block in &mut self.all_blocks {
                    unsafe {
                        (**block).evacuation_candidate =
                            (**block).hole_count as usize >= hole_threshhold;
                    }
                }
            }
        }

        match (false, perform_evac, true) {
            (true, false, true) => CollectionType::ImmixCollection,
            (true, true, true) => CollectionType::ImmixEvacCollection,
            (false, false, _) => CollectionType::ImmixCollection,
            (false, true, _) => CollectionType::ImmixEvacCollection,
            _ => CollectionType::ImmixCollection,
        }
    }

    pub fn collect(
        &mut self,
        log: bool,
        space_bitmap: &SpaceBitmap,
        collection_type: &CollectionType,
        roots: &[*mut HeapCell],
        precise_roots: &[*mut *mut HeapCell],
        immix_space: &mut ImmixSpace,
        large_object_space: &mut LargeObjectSpace,
        next_live_mark: bool,
    ) -> usize {
        // TODO: maybe use immix_space.bitmap.clear_range(immix_space.begin,immix_space.block_cursor)?
        for block in &mut self.all_blocks {
            unsafe {
                immix_space
                    .bitmap
                    .clear_range((*block) as usize, (*block) as usize + BLOCK_SIZE);
                (**block).line_map.clear_all();
            }
        }
        let visited = ImmixCollector::collect(
            collection_type,
            roots,
            precise_roots,
            immix_space,
            next_live_mark,
        );
        self.mark_histogram.clear();
        let (recyclable_blocks, free_blocks) = self.sweep_all_blocks(log, space_bitmap);
        immix_space.set_recyclable_blocks(recyclable_blocks);

        // XXX We should not use a constant here, but something that
        // XXX changes dynamically (see rcimmix: MAX heuristic).
        let evac_headroom = if USE_EVACUATION {
            EVAC_HEADROOM - immix_space.evac_headroom()
        } else {
            0
        };
        immix_space.extend_evac_headroom(free_blocks.iter().take(evac_headroom).copied());
        immix_space.return_blocks(free_blocks.iter().skip(evac_headroom).copied());
        large_object_space.sweep();
        visited
    }
    /// Sweep all blocks in the buffer after the collection.
    ///
    /// This function returns a list of recyclable blocks and a list of free
    /// blocks.
    fn sweep_all_blocks(
        &mut self,
        log: bool,
        space_bitmap: &SpaceBitmap,
    ) -> (Vec<*mut ImmixBlock>, Vec<*mut ImmixBlock>) {
        let mut unavailable_blocks = Vec::new();
        let mut recyclable_blocks = Vec::new();
        let mut free_blocks = Vec::new();
        for block in self.all_blocks.drain(..) {
            log_if!(log, "-- Sweeping block {:p}", block);
            unsafe {
                /*if (*block).needs_destruction {
                    space_bitmap.visit_unmarked_range(
                        (*block).begin() + 128,
                        (*block).begin() + 32 * 1024,
                        |object| {
                            let header = object as *mut HeapCell;
                            let ty_info = (*header).type_info();
                            if ty_info.needs_destruction {
                                let destructor = ty_info.destructor.unwrap();
                                destructor(Address::from_ptr(header));
                            }
                        },
                    );
                }*/
                maybe_sweep(log, space_bitmap, block);
            }
            if unsafe { (*block).is_empty() } {
                unsafe {
                    (*block).reset();
                }
                log_if!(log, "-- Push block {:p} into free blocks.", block);
                free_blocks.push(block);
            } else {
                unsafe {
                    (*block).count_holes();
                }
                let (holes, marked_lines) = unsafe { (*block).count_holes_and_marked_lines() };
                if self.mark_histogram.contains_key(holes) {
                    if let Some(val) = self.mark_histogram.get_mut(holes) {
                        *val += marked_lines;
                    }
                } else {
                    self.mark_histogram.insert(holes, marked_lines);
                }
                log_if!(
                    log,
                    "--- Found {} holes and {} marked lines in block {:p}",
                    holes,
                    marked_lines,
                    block
                );
                match holes {
                    0 => {
                        log_if!(log, "--- Push block {:p} into unavailable blocks", block);
                        unavailable_blocks.push(block);
                    }
                    _ => {
                        log_if!(log, "--- Push block {:p} into recyclable blocks", block);
                        recyclable_blocks.push(block);
                    }
                }
            }
        }
        self.all_blocks = unavailable_blocks;
        (recyclable_blocks, free_blocks)
    }

    /// Calculate how many holes a block needs to have to be selected as a
    /// evacuation candidate.
    fn establish_hole_threshhold(&self, evac_headroom: usize) -> usize {
        let mut available_histogram: VecMap<usize> = VecMap::with_capacity(NUM_LINES_PER_BLOCK);
        for &block in &self.all_blocks {
            let (holes, free_lines) = unsafe { (*block).count_holes_and_available_lines() };
            if available_histogram.contains_key(holes) {
                if let Some(val) = available_histogram.get_mut(holes) {
                    *val += free_lines;
                }
            } else {
                available_histogram.insert(holes, free_lines);
            }
        }
        let mut required_lines = 0;
        let mut available_lines = evac_headroom * (NUM_LINES_PER_BLOCK - 1);

        for threshold in 0..NUM_LINES_PER_BLOCK {
            required_lines += *self.mark_histogram.get(threshold).unwrap_or(&0);
            available_lines =
                available_lines.saturating_sub(*available_histogram.get(threshold).unwrap_or(&0));
            if available_lines <= required_lines {
                return threshold;
            }
        }
        NUM_LINES_PER_BLOCK
    }
}

pub unsafe fn maybe_sweep(log: bool, space_bitmap: &SpaceBitmap, block: *mut ImmixBlock) {
    log_if!(log, "--- Will sweep block?={} ", (*block).needs_destruction);
    if (*block).needs_destruction {
        space_bitmap.visit_unmarked_range(
            (*block).begin() + 128,
            (*block).begin() + 32 * 1024,
            |object| {
                let header = object as *mut HeapCell;
                let ty_info = (*header).get_dyn();
                std::ptr::drop_in_place(ty_info);
            },
        );
    }
}
