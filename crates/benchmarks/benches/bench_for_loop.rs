use criterion::{criterion_group, criterion_main, Criterion};
use gc::handle::Handle;
use runtime::{arguments::Arguments, value::JsValue};
use starlight::*;
use vm::{Options, VirtualMachine};

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
const CODE: &'static str = r#"
for (let i = 0;i<10000;i = i + 1) {

}

"#;
fn criterion_benchmark(c: &mut Criterion) {
    let mut vm = VirtualMachine::new(Options::default());
    // vm.space().defer_gc();
    let mut func = vm
        .compile(false, CODE, "<Code>")
        .unwrap_or_else(|_| panic!())
        .root();
    let mut boa_ctx = boa::Context::new();
    let args = Arguments::new(&mut vm, JsValue::undefined(), 0);
    let mut args = Handle::new(vm.space(), args);
    c.bench_function("starlight-for-loop", |b| {
        b.iter(|| match func.as_function_mut().call(&mut vm, &mut args) {
            Ok(_) => (),
            Err(_) => unreachable!(),
        });
    });

    c.bench_function("boa-eval-for-loop", |b| {
        b.iter(|| {
            boa_ctx.eval(CODE).unwrap();
        });
    });
}
