use starlight::vm::VirtualMachine;
use starlight::{
    jsrt::jsrt_init,
    vm::{Options, VirtualMachineRef},
};
use structopt::StructOpt;

fn main() {
    let mut vm = VirtualMachine::new(Options::from_args());
    jsrt_init(&mut vm);
    let res = vm.eval(
        true,
        r#""use strict";
let i = 0
while (i < 10) {
    print(i)
    i = i + 1
}


        "#,
    );
    match res {
        Ok(_) => {
            println!("done");
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
