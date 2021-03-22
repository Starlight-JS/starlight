use criterion::{black_box, criterion_group, criterion_main, Criterion};
use starlight::{
    gc::{
        cell::{GcCell, Trace},
        default_heap,
        snapshot::serializer::{Serializable, SnapshotSerializer},
        Heap,
    },
    vm::GcParams,
    vtable_impl,
};
use wtf_rs::keep_on_stack;
struct Large([u8; 8192]);
unsafe impl Trace for Large {}
impl GcCell for Large {
    fn deser_pair(&self) -> (usize, usize) {
        (0, 0)
    }
    vtable_impl!();
}

impl Serializable for Large {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {}
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut gc = default_heap(GcParams::default());
    //gc.defer();
    c.bench_function("bench-alloc-f64", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                let x = black_box(gc.allocate(42.42));
                keep_on_stack!(&x);
                let y = black_box(gc.allocate(42.42));
                keep_on_stack!(&y);
                let z = black_box(gc.allocate(42.42));
                keep_on_stack!(&z);
            }
        });
    });
    //  gc.undefer();
    gc.collect_if_necessary();
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
