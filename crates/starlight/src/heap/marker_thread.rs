use super::{Heap, SlotVisitor};

// TODO

pub struct Marker {
    visitor: SlotVisitor,
    /// Heap does not have static lifetime but transmuted lifetime so we can use `Marker` more easily.
    heap: &'static mut Heap,
}

impl Marker {}
