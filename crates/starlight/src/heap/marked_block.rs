use crate::vm::RuntimeRef;

use super::{block_directory::BlockDirectory, subspace::Subspace, weak_set::WeakSet};

pub type HeapVersion = u32;

// A marked block is a page-aligned container for heap-allocated objects.
// Objects are allocated within cells of the marked block. For a given
// marked block, all cells have the same size. Objects smaller than the
// cell size may be allocated in the marked block, in which case the
// allocation suffers from internal fragmentation: wasted space whose
// size is equal to the difference between the cell size and the object
// size.
pub struct MarkedBlock;

pub const ATOM_SIZE: usize = 16;
pub const BLOCK_SIZE: usize = 16 * 1024;
pub const BLOCK_MASK: usize = !(BLOCK_SIZE - 1);
pub const ATOMS_PER_BLOCK: usize = BLOCK_SIZE / ATOM_SIZE;
pub const MAX_NUMBER_OF_LOWER_TIER_CELLS: usize = 8;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmptyMode {
    IsEmpty,
    NotEmpty,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NewlyAllocatedMode {
    HasNewlyAllocated,
    DoesNotHaveNewlyAllocated,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarksMode {
    MarksStale,
    MarksNotStale,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SweepDestructionMode {
    BlockHasNoDestructors,
    BlockHasDestructors,
    BlockHasDestructorsAndCollectorIsRunning,
}

#[repr(C)]
pub struct MarkedBlockHandle {
    heap: *mut super::Heap,
    weak_set: WeakSet,
    block: *mut MarkedBlock,
    directory: *mut BlockDirectory,
    index: usize,
    is_freelisted: bool,
    atoms_per_cell: usize,
    end_atom: usize,
}

impl MarkedBlockHandle {
    pub fn is_freelisted(&self) -> bool {
        self.is_freelisted
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

#[allow(dead_code)]
pub struct MarkedBlockFooter {
    handle: &'static mut MarkedBlockHandle,
    vm: RuntimeRef,
    subspace: *mut Subspace,

    // The actual mark count can be computed by doing: biasedMarkCount - markCountBias. Note
    // that this count is racy. It will accurately detect whether or not exactly zero things were
    // marked, but if N things got marked, then this may report anything in the range [1, N] (or
    // before unbiased, it would be [1 + m_markCountBias, N + m_markCountBias].)
    biased_mark_count: i16,
    // We bias the mark count so that if m_biasedMarkCount >= 0 then the block should be retired.
    // We go to all this trouble to make marking a bit faster: this way, marking knows when to
    // retire a block using a js/jns on m_biasedMarkCount.
    //
    // For example, if a block has room for 100 objects and retirement happens whenever 90% are
    // live, then m_markCountBias will be -90. This way, when marking begins, this will cause us to
    // set m_biasedMarkCount to -90 as well, since:
    //
    //     m_biasedMarkCount = actualMarkCount + m_markCountBias.
    //
    // Marking an object will increment m_biasedMarkCount. Once 90 objects get marked, we will have
    // m_biasedMarkCount = 0, which will trigger retirement. In other words, we want to set
    // m_markCountBias like so:
    //
    //     m_markCountBias = -(minMarkedBlockUtilization * cellsPerBlock)
    //
    // All of this also means that you can detect if any objects are marked by doing:
    //
    //     m_biasedMarkCount != m_markCountBias
    mark_count_bias: i16,
}
