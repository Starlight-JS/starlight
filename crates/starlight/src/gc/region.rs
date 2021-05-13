//! Region based garbage collector.
//!
//!
//! This GC is using Immix algorithm for small enough objects (Fits under 32KB blocks) and mimalloc for others.
//! Allocation for small objects in almost all cases is simple bump-pointer.
//!
//!
//!
//! ## To move or not to move
//!
//! RegionGC does not yet implement moving but we can add it in future. Main problem is performance because we have to track pointers
//! that is found on stack convservatively and can't be moved, and including our `letroot!` functionlaity this makes evacuation almost useless.
pub mod allocator;
pub mod block;
pub mod block_allocator;
