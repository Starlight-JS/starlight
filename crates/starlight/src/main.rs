use std::{
    fmt::Display,
    io::{stderr, Write},
    sync::RwLock,
};

use starlight::{
    bytecode::opcodes::Op,
    frontend::Compiler,
    jsrt::jsrt_init,
    runtime::{
        arguments::Arguments,
        function::JsVMFunction,
        object::{object_size_with_tag, JsObject},
        value::JsValue,
    },
    vm::{Options, VirtualMachineRef},
};
use starlight::{bytecode::ByteCodeBuilder, vm::VirtualMachine};
use structopt::StructOpt;

fn main() {
    let mut vm = VirtualMachine::new(Options::from_args());
    jsrt_init(&mut vm);
    let res = vm.eval(
        r#"
foo()

{
    function foo() {
        print("Hi!")
    }
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
