use std::fs::read_to_string;

use starlight::{
    prelude::{Arguments, GcPointer, JsValue, Options},
    vm::context::Context,
    Platform,
};

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let ctx = runtime.new_context();

    let content = read_to_string("examples/hello-world.js").unwrap();
    let res = ctx.eval_internal(None, false, &content, false);

    match res {
        Ok(val) => {
            val.to_string(ctx).unwrap_or_else(|_| String::new());
        }
        Err(e) => println!(
            "Uncaught {}",
            e.to_string(ctx).unwrap_or_else(|_| String::new())
        ),
    };
}
