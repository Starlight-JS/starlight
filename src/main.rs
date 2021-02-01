use std::mem::size_of;

use js::runtime::ref_ptr::Ref;
use js::{
    gc::bitmap::BitMap,
    runtime::{js_string::JsString, options::Options},
};
use js::{heap::block::ImmixBlock, runtime::vm::JsVirtualMachine};
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
    foo(vm);
    vm.gc(false);

    println!("{}", my_str.as_str());
}
