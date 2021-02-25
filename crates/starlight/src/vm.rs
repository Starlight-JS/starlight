use crate::heap::Heap;
#[macro_use]
pub mod class;
#[macro_use]
pub mod method_table;
pub mod array_storage;
pub mod attributes;
pub mod object;
pub mod property_descriptor;
pub mod slot;
pub mod string;
pub mod structure;
pub mod symbol_table;
pub mod thread;
pub mod value;
pub struct Runtime {
    heap: Box<Heap>,
}

impl Runtime {
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.heap
    }

    pub fn new(track_allocations: bool) -> Box<Runtime> {
        let heap = Box::new(Heap::new(track_allocations));
        Box::new(Self { heap })
    }
}
