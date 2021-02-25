use criterion::{criterion_group, criterion_main, Criterion};
use starlight::vm::symbol_table::SymbolTable;
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

pub fn criterion_benchmark(c: &mut Criterion) {
    let table = SymbolTable::new();
    c.bench_function("intern", |b| {
        b.iter(|| {
            for i in 0..10 {
                table.intern(i.to_string());
            }
        });
    });
}
