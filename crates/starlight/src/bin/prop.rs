use starlight::{
    prelude::{JsValue, Options},
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
        .unwrap()
        .to_object(ctx)
        .unwrap();

    let structure = ctx.global_data().get_function_struct();
    let prototype = structure.prototype();

    println!("{:?}", JsValue::new(structure));
    println!("{:?}", JsValue::new(*prototype.unwrap()));

    let structure = func.structure();
    let prototype = structure.prototype();

    println!("{:?}", JsValue::new(structure));

    println!("{:?}", JsValue::new(*prototype.unwrap()));
    ctx.eval("print(Object.prototype.hasOwnProperty)").unwrap();
}
