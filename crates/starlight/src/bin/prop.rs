use std::fs::read_to_string;

<<<<<<< HEAD
use starlight::{prelude::Options, Platform};
=======
use starlight::{
    prelude::{Options},
    Platform,
};
>>>>>>> 0f74711ac64be8d31edf8cd0be7251064569679f

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
