use crate::vm::Runtime;

pub mod deserializer;
pub mod serializer;

pub struct Snapshot {
    pub buffer: Box<[u8]>,
}

impl Snapshot {
    pub fn take(runtime: &mut Runtime) -> Self {
        let mut serializer = serializer::SnapshotSerializer::new();

        serializer.build_reference_map(runtime);
        serializer.build_symbol_table();
        serializer.build_heap_reference_map(runtime);
        serializer.serialize(runtime);
        Snapshot {
            buffer: serializer.output.into_boxed_slice(),
        }
    }
}
