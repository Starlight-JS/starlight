use criterion::{criterion_group, criterion_main, Criterion};
use starlight::{
    gc::default_heap,
    gc::snapshot::{deserializer::Deserializer, Snapshot},
    prelude::Options,
    vm::Runtime,
    Platform,
};
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn criterion_benchmark(c: &mut Criterion) {
    Platform::initialize();
    let options = Options::default();
    let mut initial_rt = Runtime::new(options, None);
    let snapshot = Snapshot::take(false, &mut initial_rt, |_, _| {})
        .buffer
        .to_vec();

    c.bench_function("runtime from scratch", |b| {
        b.iter_with_large_drop(|| Runtime::new(Options::default(), None));
    });

    c.bench_function("runtime from snapshot", |b| {
        b.iter_with_large_drop(|| {
            let opts = Options::default();
            let heap = default_heap(&opts);
            Deserializer::deserialize(false, &snapshot, opts, heap, None, |_, _| {})
        });
    });
}
