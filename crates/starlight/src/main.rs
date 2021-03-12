use starlight::{
    heap::snapshot::{deserializer, Snapshot},
    vm::Runtime,
    Platform,
};

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(false, None);
    rt.heap().gc();
    let snapshot = Snapshot::take(true, &mut rt);
    drop(rt);
    let _deserialized_rt = deserializer::Deserializer::deserialize(true, &snapshot.buffer, None);
}
