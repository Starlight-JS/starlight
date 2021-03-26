use criterion::{black_box, criterion_group, criterion_main, Criterion};
use starlight::{
    gc::{
        cell::{GcCell, Trace},
        default_heap,
        shadowstack::ShadowStack,
        snapshot::serializer::{Serializable, SnapshotSerializer},
        SimpleMarkingConstraint,
    },
    root,
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
    let mut gc = default_heap(GcParams::default().with_parallel_marking(false));
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
                root!(x = stack, black_box(gc.allocate(42.42)));
                keep_on_stack!(&x);
                root!(y = stack, black_box(gc.allocate(42.42)));
                keep_on_stack!(&y);
                root!(z = stack, black_box(gc.allocate(42.42)));
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
