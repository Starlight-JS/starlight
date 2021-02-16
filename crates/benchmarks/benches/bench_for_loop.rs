use criterion::{black_box, criterion_group, criterion_main, Criterion};
use runtime::{
    object::{JsObject, ObjectTag},
    structure::Structure,
    symbol::Symbol,
    value::JsValue,
};
use starlight::*;
use vm::{Options, VirtualMachine};

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn criterion_benchmark(c: &mut Criterion) {
    let mut vm = VirtualMachine::new();
}
