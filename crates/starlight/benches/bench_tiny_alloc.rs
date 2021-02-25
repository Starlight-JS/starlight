use criterion::{black_box, criterion_group, criterion_main, Criterion};
use starlight::heap::{
    cell::{GcCell, Trace},
    usable_size, Heap,
};
use wtf_rs::keep_on_stack;
struct Large([u8; 8192]);
unsafe impl Trace for Large {}
impl GcCell for Large {}
pub fn criterion_benchmark(c: &mut Criterion) {
    let mut heap = Heap::new(false);
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

    let my_small_alloc = heap.allocate(42);

    c.bench_function("usable-size-small", |b| {
        b.iter(|| {
            assert!(black_box(usable_size(my_small_alloc)) >= 16);
        });
    });

    let larger = heap.allocate(Large([0; 8192]));

    c.bench_function("usable-size-large", |b| {
        b.iter(|| {
            assert!(black_box(usable_size(larger)) >= 8192);
        });
    });
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
