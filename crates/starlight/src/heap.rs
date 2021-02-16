use cell::{Cell, Gc};

pub mod addr;
pub mod bitmap;
pub mod block;
pub mod block_set;
pub mod cell;
pub mod constraint;
pub mod context;
pub mod precise_allocation;
//pub mod space;
pub mod tiny_bloom_filter;
#[cfg(feature = "debug-snapshots")]
use cell::Header;
#[cfg(feature = "debug-snapshots")]
use std::fs::File;

use crate::gc::heap::Heap;
#[cfg(feature = "debug-snapshots")]
pub fn freeze_cell_into(hdr: *const Header, cell: &dyn Cell, mut file: &mut File) {
    use serde::{Deserialize, Serialize, Serializer};
    use serde_reflection::{Registry, Samples, Tracer, TracerConfig};

    use std::io::Write;
    file.write_all(format!("at {:p}: ", hdr).as_bytes())
        .unwrap();
    erased_serde::serialize(
        cell,
        &mut ron::Serializer::new(&mut file, None, true).unwrap(),
    )
    .unwrap();
    file.write("\n".as_bytes()).unwrap();
}

pub trait Allocator<T> {
    type Result;

    fn allocate(&mut self, value: T) -> Self::Result;
}

impl<T: Cell> Allocator<T> for Heap {
    type Result = Gc<T>;

    fn allocate(&mut self, value: T) -> Self::Result {
        self.alloc(value)
    }
}
