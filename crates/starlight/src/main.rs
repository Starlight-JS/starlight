use starlight::{
    heap::snapshot::{deserializer, Snapshot},
    vm::{GcParams, Runtime},
    Platform,
};

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(GcParams::default().with_parallel_marking(true), None);
    rt.heap().gc();

    let snapshot = Snapshot::take(!true, &mut rt);
    drop(rt);
    std::fs::write("snapshot.out", &snapshot.buffer).unwrap();
    let mut rt =
        deserializer::Deserializer::deserialize(!true, &snapshot.buffer, GcParams::default(), None);

    println!("{}", rt.heap().allocated());
}
