use std::fs::read_to_string;

use starlight::{
    prelude::{Arguments, GcPointer, JsNativeFunction, JsValue, Options},
    vm::context::Context,
    Platform,
};

fn load(mut ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let filename = "octane/".to_string() + &args.at(0).to_string(ctx)?;
    println!("Load {}", filename);
    let source = read_to_string(filename).unwrap();
    ctx.eval(&source)?;
    Ok(JsValue::UNDEFINED)
}

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default(), None);
    let mut ctx = runtime.new_context();

    let load = JsNativeFunction::new(ctx, "load", load, 1);

    ctx.global_object().put(ctx, "load", load, false).unwrap();
    ctx.global_object()
        .put(ctx, "performance", JsValue::UNDEFINED, false)
        .unwrap();

    let source = read_to_string("octane/run.js").unwrap();

    match ctx.eval(&source) {
        Ok(_) => {}
        Err(e) => {
            println!("{:?}", e.to_string(ctx).unwrap());
        }
    };
}
