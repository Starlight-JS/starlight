use std::time::Instant;

use starlight::options::Options;
use starlight::prelude::{Deserializer, Snapshot};
use starlight::vm::context::Context;
use starlight::Platform;

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let ctx = Context::new(&mut runtime);

    runtime.heap().defer();

    let buffer = Snapshot::take_context(false, &mut runtime, ctx, |_, _| {})
        .buffer
        .to_vec();

    runtime.remove_context(ctx);

    runtime.heap().undefer();

    let start = Instant::now();
    let mut ctx = Deserializer::deserialize_context(&mut runtime, false, &buffer);
    println!("Deserialize context cost: {:?}", start.elapsed());
    ctx.eval("print('hello,world');print=null;").unwrap();

    let start = Instant::now();
    let mut ctx = Context::new(&mut runtime);
    println!("Init context cost: {:?}", start.elapsed());
    ctx.eval("print('hello,world');print=null").unwrap();

    let start = Instant::now();
    let mut ctx = runtime.new_context();
    println!("Init context cost: {:?}", start.elapsed());
    ctx.eval("print('hello,world');print=null").unwrap();

    let start = Instant::now();
    let mut ctx = runtime.new_context();
    println!("Init context cost: {:?}", start.elapsed());
    ctx.eval("print('hello,world');print=null").unwrap();
    unsafe {
        runtime.dispose();
    }
}
