use starlight::{
    gc::handle::Handle,
    jsrt::jsrt_init,
    vm::{Options, VirtualMachineRef},
};
use starlight::{
    runtime::{arguments::Arguments, value::JsValue},
    vm::VirtualMachine,
};
use structopt::StructOpt;
const CODE: &'static str = r#"
function f(i) {}
for (let i = 0;i<1000;i = i + 1) {
    f(i)
}

"#;
fn main() {
    let mut vm = VirtualMachine::new(Options::from_args());
    jsrt_init(&mut vm);
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
    VirtualMachineRef::dispose(vm);
}
