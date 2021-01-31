use js::runtime::vm::JSVirtualMachine;
use js::runtime::{js_string::JSString, options::Options};
use structopt::StructOpt;
fn main() {
    let mut vm = JSVirtualMachine::create(Options::from_args());
    let string = JSString::new(&mut vm, "Hello,World!");
    vm.gc(true);
}
