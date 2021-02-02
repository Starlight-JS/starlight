use starlight_vm::runtime::vm::JsVirtualMachine;
use starlight_vm::runtime::{js_string::JsString, options::Options};
use starlight_vm::{runtime::ref_ptr::Ref, util::array::GcVec};
use structopt::StructOpt;

#[inline(never)]
fn foo(vm: Ref<JsVirtualMachine>) {
    let mut  _larger_string = Some(JsString::new(
        vm,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbcccccccccccccccccccccccccccccccdddddddddddddd",
    ));
    _larger_string = None;
}
fn clp2(number: usize) -> usize {
    let x = number - 1;
    let x = x | (x >> 1);
    let x = x | (x >> 2);
    let x = x | (x >> 4);
    let x = x | (x >> 8);
    let x = x | (x >> 16);
    x + 1
}
fn main() {
    /*let mut vm = JsVirtualMachine::create(Options::from_args());
    let my_str = JsString::new(vm, "Hello,World!");
    let mut vec = GcVec::new(vm, 1);
    vec.push(vm, my_str);
    vec.push(vm, JsString::new(vm, "Hi!"));
    foo(vm);
    vm.gc(false);

    println!("{}", vec.pop().unwrap().as_str());
    println!("{:p}", my_str.cell);
    println!("{}", vec.pop().unwrap().as_str());*/
    println!("{}", clp2(18));
}
