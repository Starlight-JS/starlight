use crate::gc::cell::{Cell, Gc};
use crate::gc::heap::Heap;
pub mod addr;

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
