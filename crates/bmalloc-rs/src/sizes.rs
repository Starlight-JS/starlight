//! Module for malloc sizing constants and calculations.

pub const KB: usize = 1024;
pub const MB: usize = KB * KB;
pub const GB: usize = KB * KB * KB;
pub const ALIGNMENT: usize = 8;
pub const ALIGNMENT_MASK: usize = ALIGNMENT - 1;
pub const CHUNK_SIZE: usize = 1 * MB;
pub const CHUNK_MASK: usize = !(CHUNK_SIZE - 1);
pub const SMALL_LINE_SIZE: usize = 256;
pub const SMALL_PAGE_SIZE: usize = 4096;
pub const SMALL_PAGE_LINE_COUNT: usize = SMALL_PAGE_SIZE / SMALL_LINE_SIZE;
pub const MASK_SIZE_CLASS_MAX: usize = 512;
pub const SMALL_MAX: usize = 32 * KB;
pub const PAGE_SIZE_MAX: usize = SMALL_MAX * 2;
pub const PAGE_CLASS_COUNT: usize = PAGE_SIZE_MAX / SMALL_PAGE_SIZE;
pub const PAGE_SIZE_WASTE_FACTOR: usize = 8;
pub const LOG_WASTE_FACTOR: usize = 8;
pub const LARGE_ALIGNMENT: usize = SMALL_MAX / PAGE_SIZE_WASTE_FACTOR;
pub const LARGE_ALIGNMENT_MASK: usize = LARGE_ALIGNMENT - 1;
pub const DEALLOCATOR_LOG_CAPACITY: usize = 512;
pub const BUMP_RANGE_CACHE_CAPACITY: usize = 3;
pub const SCAVENGE_BYTES_PER_MEMORY_PRESSURE_CHECK: usize = 16 * MB;
pub const MEMORY_PRESSURE_THRESHOLD: f64 = 0.75;
