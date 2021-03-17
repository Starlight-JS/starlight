use starlight::{
    vm::{GcParams, Runtime, RuntimeParams},
    Platform,
};
const SRC: &'static str = r#"
function factorial(x) {
    if (x < 2) {
        return x;
    } else {
        return factorial(x - 1) * x;
    }
}

throw new TypeError("Error");
"#;
fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(
        RuntimeParams::default().with_dump_bytecode(true),
        GcParams::default().with_parallel_marking(true),
        None,
    );

    match rt.eval(false, SRC) {
        Ok(_) => {}
        Err(e) => {
            let e = e.to_string(&mut rt).unwrap_or_else(|_| panic!());
            eprintln!("{}", e);
            return;
        }
    }
    println!("{}", rt.get_global("x").unwrap().get_number());
    println!("{}", rt.heap().threshold());
    //assert!(rt.get_global("x").unwrap().get_number() == 3.0);
}
