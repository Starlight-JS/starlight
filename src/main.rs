use std::mem::size_of;

use js::runtime::{js_string::JSString, options::Options};
use js::{heap::block::ImmixBlock, runtime::vm::JSVirtualMachine};
use structopt::StructOpt;
fn main() {
    let mut vm = JSVirtualMachine::create(Options::from_args());
    let string = JSString::new(&mut vm, "Hello,World!");
    println!("{:p}", string.pointer);
    vm.gc(true);
    assert_eq!(string.vm().pointer, vm.pointer);
}
