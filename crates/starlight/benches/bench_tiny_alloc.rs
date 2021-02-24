use criterion::{black_box, criterion_group, criterion_main, Criterion};
use starlight::heap::Heap;
use wtf_rs::keep_on_stack;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut heap = Heap::new();
    //heap.defer();
    c.bench_function("bench-alloc-f64", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                let x = black_box(heap.allocate(42.42));
                keep_on_stack!(&x);
                let y = black_box(heap.allocate(42.42));
                keep_on_stack!(&y);
                let z = black_box(heap.allocate(42.42));
                keep_on_stack!(&z);
            }
        });
    });
    //  heap.undefer();
    heap.collect_if_necessary();
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
