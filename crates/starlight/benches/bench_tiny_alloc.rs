use criterion::{black_box, criterion_group, criterion_main, Criterion};
use starlight::{
    gc::{
        cell::{GcCell, Trace},
        default_heap,
        shadowstack::ShadowStack,
        snapshot::serializer::{Serializable, SnapshotSerializer},
        SimpleMarkingConstraint,
    },
    letroot,
    prelude::Options,
};
use wtf_rs::keep_on_stack;
struct Large([u8; 8192]);
impl Trace for Large {}
impl GcCell for Large {
    fn deser_pair(&self) -> (usize, usize) {
        (0, 0)
    }
}

impl Serializable for Large {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {}
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut gc = default_heap(&Options::default());
    //gc.defer();
    let mut stack = Box::new(ShadowStack::new());
    let stack_ptr: *mut ShadowStack = &mut *stack;
    gc.add_constraint(SimpleMarkingConstraint::new(
        "mark-stack",
        move |visitor| unsafe {
            (*stack_ptr).trace(visitor);
        },
    ));
    c.bench_function("bench-alloc-f64", |b| {
        b.iter(|| {
            for _ in 0..10000 {
                letroot!(x = stack, black_box(gc.allocate(42.42)));
                keep_on_stack!(&x);
                letroot!(y = stack, black_box(gc.allocate(42.42)));
                keep_on_stack!(&y);
                letroot!(z = stack, black_box(gc.allocate(42.42)));
                keep_on_stack!(&z);
                gc.collect_if_necessary();
            }
        });
    });
    //  gc.undefer();
    gc.collect_if_necessary();
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
