use std::fs::read_to_string;

use starlight::{
    prelude::{JsHint, Options},
    Platform,
};

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let mut ctx = runtime.new_context();

    let prototype = ctx
        .global_object()
        .get(ctx, "Object")
        .unwrap()
        .to_object(ctx)
        .unwrap()
        .get(ctx, "prototype")
        .unwrap();
    let func = prototype
        .to_object(ctx)
        .unwrap()
        .get(ctx, "hasOwnProperty")
        .unwrap();
    println!("{:?}", &func);

    ctx.eval("print(Object.prototype.hasOwnProperty)").unwrap();
}
