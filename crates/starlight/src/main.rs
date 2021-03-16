use starlight::{
    heap::snapshot::{deserializer, Snapshot},
    vm::{GcParams, Runtime, RuntimeParams},
    Platform,
};
const SRC: &'static str = r#"
const add = (x,y) => x + y
x = add(1,2)
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
        Err(_) => {
            panic!("eval failed");
        }
    }
    println!("{}", rt.get_global("x").unwrap().get_number());
    println!("{}", rt.heap().threshold());
    //assert!(rt.get_global("x").unwrap().get_number() == 3.0);
}
