use std::fs::read_to_string;

use starlight::{Platform, prelude::{Options}};

fn main() {
    Platform::initialize();
    let mut runtime = Platform::new_runtime(Options::default().with_dump_bytecode(true), None);
    let mut ctx = runtime.new_context();

    let source = read_to_string("examples/hello-world.js").unwrap();
    match ctx.eval(&source) {
        Ok(_) => {}
        Err(e) => {
            println!("{:?}", e.to_string(ctx).unwrap());
        }
    };
}
