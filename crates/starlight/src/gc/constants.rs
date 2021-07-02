pub const BLOCK_SIZE: usize = 16 * 1024;

pub const LINE_SIZE: usize = 128;

pub const NUM_LINES_PER_BLOCK: usize = BLOCK_SIZE / LINE_SIZE;
// Objects smaller than MEDIUM_OBJECT are allocated with the
/// `NormalAllocator`, otherwise the `OverflowAllocator` is used.
pub const MEDIUM_OBJECT: usize = LINE_SIZE;

/// Objects larger than LARGE_OBJECT are allocated using the `LargeObjectSpace`.
pub const LARGE_OBJECT: usize = 8 * 1024;
