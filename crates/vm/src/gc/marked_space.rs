use std::collections::{HashSet, LinkedList};

use crate::heap::{constants::LARGE_OBJECT, large_object_space::PreciseAllocation, util::round_up};

use super::{block_directory::*, marked_block::*, marked_block_set::*};
/// sizeStep is really a synonym for atomSize; it's no accident that they are the same.
pub const PRECISE_CUTOFF: usize = 80;
/// Sizes up to this amount get a size class for each size step.
pub const STEP_SIZE: usize = 16;
/// The amount of available payload in a block is the block's size minus the footer.
pub const BLOCK_PAYLOAD: usize = PAYLOAD_SIZE;
/// The largest cell we're willing to allocate in a MarkedBlock the "normal way" (i.e. using size
/// classes, rather than a large allocation) is half the size of the payload, rounded down. This
/// ensures that we only use the size class approach if it means being able to pack two things
/// into one block.
pub const LARGE_CUTOFF: usize = (BLOCK_PAYLOAD / 2) & !(STEP_SIZE - 1);
/// We have an extra size class for size zero.
pub const NUM_SIZE_CLASSES: usize = LARGE_CUTOFF / STEP_SIZE + 1;

pub const fn size_class_to_index(size: usize) -> usize {
    (size + STEP_SIZE - 1) / STEP_SIZE
}

pub const fn index_to_size_class(index: usize) -> usize {
    index * STEP_SIZE
}

pub struct MarkedSpace {
    precise_allocation_set: HashSet<*mut PreciseAllocation>,
    precise_allocations: Vec<*mut PreciseAllocation>,
    precise_allocations_nursery_offset: u32,
    precise_allocations_offset_for_this_collection: u32,
    precise_allocations_offset_for_sweep: u32,
    precise_allocations_offset_for_this_collection_size: u32,
    precise_allocations_for_this_collection_begin: *mut *mut PreciseAllocation,
    precise_allocations_for_this_collection_end: *mut *mut PreciseAllocation,

    capacity: usize,

    blocks: MarkedBlockSet,
    directories: LinkedList<Box<BlockDirectory>>,
    pub(super) size_class_for_size_step: [u32; NUM_SIZE_CLASSES],
}

fn size_classes() -> Vec<usize> {
    let mut result = vec![];
    let mut add = |res: &mut Vec<usize>, mut sz| {
        sz = round_up(sz as _, 16) as usize;
        res.push(sz);
    };
    let mut size = STEP_SIZE;
    while size < PRECISE_CUTOFF {
        add(&mut result, size);
        size += STEP_SIZE;
    }

    for i in 0.. {
        let approximate_size = PRECISE_CUTOFF as f64 * (1.4f64.powi(i));
        let approximate_size_in_bytes = approximate_size as usize;
        if approximate_size_in_bytes > LARGE_CUTOFF {
            break;
        }

        let size_class = round_up(approximate_size_in_bytes as _, 16) as usize;
        let cells_per_block = BLOCK_PAYLOAD / size_class;
        let possibly_better_size_class = (BLOCK_PAYLOAD / cells_per_block) & !(STEP_SIZE - 1);
        let original_wastage = BLOCK_PAYLOAD - cells_per_block * size_class;
        let new_wastage = (possibly_better_size_class - size_class) * cells_per_block;
        let better_size_class = if new_wastage > original_wastage {
            size_class
        } else {
            possibly_better_size_class
        };

        if Some(better_size_class) == result.last().copied() {
            continue;
        }

        if better_size_class > LARGE_CUTOFF {
            break;
        }
        add(&mut result, better_size_class);
    }
    result
}
