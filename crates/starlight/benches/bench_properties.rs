use criterion::{black_box, criterion_group, criterion_main, Criterion};
use runtime::{
    object::{JsObject, ObjectTag},
    structure::Structure,
    symbol::Symbol,
    value::JsValue,
};
use starlight::*;
use vm::{Options, VirtualMachine};

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut vm = VirtualMachine::new(Options::default());
    let s = Structure::new_indexed(&mut vm, None, false);
    let mut obj = JsObject::new(&mut vm, s, JsObject::get_class(), ObjectTag::Ordinary);
    let ctx = vm.space().new_local_context();
    let sym = vm.intern("x");
    let local = ctx.new_local(obj);
    c.bench_function("property set", |b| {
        b.iter(|| {
            let _ = obj.put(&mut vm, sym, JsValue::new(42), false);
        })
    });
    c.bench_function("property load", |b| {
        b.iter(|| match black_box(obj.get(&mut vm, sym)) {
            Ok(_) => (),
            Err(_) => unreachable!(),
        })
    });

    c.bench_function("property set indexed", |b| {
        b.iter(|| {
            let _ = obj.put(&mut vm, Symbol::Indexed(5), JsValue::new(42), false);
        })
    });
    c.bench_function("property load indexed", |b| {
        b.iter(|| match black_box(obj.get(&mut vm, Symbol::Indexed(5))) {
            Ok(_) => (),
            Err(_) => unreachable!(),
        })
    });
    c.bench_function("property set indexed sparse", |b| {
        b.iter(|| {
            let _ = obj.put(
                &mut vm,
                Symbol::Indexed((1024 << 6) + 1),
                JsValue::new(42),
                false,
            );
        })
    });
    c.bench_function("property load indexed sparse", |b| {
        b.iter(
            || match black_box(obj.get(&mut vm, Symbol::Indexed((1024 << 6) + 1))) {
                Ok(_) => (),
                Err(_) => unreachable!(),
            },
        )
    });
    drop(local);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
