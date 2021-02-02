use std::mem::size_of;

use starlight_vm::{
    gc::bitmap::BitMap,
    runtime::{js_string::JsString, options::Options},
};
use starlight_vm::{heap::block::ImmixBlock, runtime::vm::JsVirtualMachine};
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
fn main() {
    let mut vm = JsVirtualMachine::create(Options::from_args());
    let my_str = JsString::new(vm, "Hello,World!");
    let mut vec = GcVec::new(vm, 1);
    vec.push(vm, my_str);
    vec.push(vm, JsString::new(vm, "Hi!"));
    foo(vm);
    vm.gc(false);

    println!("{}", vec.pop().unwrap().as_str());
    println!("{}", vec.pop().unwrap().as_str());

    unsafe {
        vm.get_all_live_objects(|pointer| {
            println!("{:p}", pointer);
        })
    }
}
