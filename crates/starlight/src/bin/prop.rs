use std::fs::read_to_string;

use starlight::{
    prelude::{Arguments, DataDescriptor, GcPointer, Internable, JsObject, JsValue, Options, C, W},
    vm::context::Context,
    Platform,
};

fn nop(_ctx: GcPointer<Context>, _args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::encode_undefined_value())
}

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default().with_dump_bytecode(true), None);
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
