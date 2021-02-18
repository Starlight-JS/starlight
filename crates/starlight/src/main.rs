use starlight::{
    gc::{formatted_size, handle::Handle},
    jsrt::jsrt_init,
    vm::{Options, VirtualMachineRef},
};
use starlight::{
    runtime::{arguments::Arguments, value::JsValue},
    vm::VirtualMachine,
};

const CODE: &'static str = r#"
var obj = new Object()

print(obj)
"#;
fn main() {
    let mut vm = VirtualMachine::new(Options::default());
    jsrt_init(&mut vm);
    println!(
        "In use heap after JSRT init {}",
        formatted_size(vm.space().heap_usage())
    );
    vm.space().gc();
    println!(
        "In use heap after JSRT init and GC {}",
        formatted_size(vm.space().heap_usage())
    );

    let res = vm.compile(false, CODE, "<Code>");
    match res {
        Ok(val) => {
            let mut fun = val.root();
            let args = Arguments::new(&mut vm, JsValue::undefined(), 0);
            let mut args = Handle::new(vm.space(), args);

            for i in 0..100 {
                println!("{}", i);
                vm.space().gc();
                match fun.as_function_mut().call(&mut vm, &mut args) {
                    Ok(_) => (),
                    Err(e) => {
                        println!(
                            "{}",
                            e.to_string(&mut vm).unwrap_or_else(|_| "shit".to_string())
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!(
                "{}",
                e.to_string(&mut vm).unwrap_or_else(|_| "shit".to_string())
            );
        }
    }
    vm.space().gc();
    println!(
        "In use heap after program {}",
        formatted_size(vm.space().heap_usage())
    );
    VirtualMachineRef::dispose(vm);
}
