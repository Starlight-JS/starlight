use std::mem::size_of;

use js_vm::{
    gc::bitmap::BitMap,
    runtime::{js_string::JsString, options::Options},
};
use js_vm::{heap::block::ImmixBlock, runtime::vm::JsVirtualMachine};
use js_vm::{runtime::ref_ptr::Ref, util::array::GcVec};
use structopt::StructOpt;
#[inline(never)]
fn foo(vm: Ref<JsVirtualMachine>) {
    let mut  _larger_string = Some(JsString::new(
        vm,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbcccccccccccccccccccccccccccccccdddddddddddddd",
    ));
    _larger_string = None;
}
fn main() {
    let mut vm = JsVirtualMachine::create(Options::from_args());
    let my_str = JsString::new(vm, "Hello,World!");
    let mut vec = GcVec::new(vm, 1);
    vec.push(vm, my_str);
    foo(vm);
    vm.gc(false);

    println!("{}", vec.pop().unwrap().as_str());
}
