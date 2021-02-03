use starlight_vm::runtime::{
    js_object::{JsObject, ObjectTag},
    js_value::JsValue,
    structure::Structure,
    vm::JsVirtualMachine,
};
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
    let mut vm = JsVirtualMachine::create(Options::from_args());
    let ctx = vm.make_context();
    let my_struct = Structure::new_(vm, &[]);
    let mut obj = JsObject::new(
        &mut vm,
        my_struct,
        JsObject::get_class(),
        ObjectTag::Ordinary,
    );

    let _ = obj.put(ctx, vm.intern("x"), JsValue::new_int(42), false);
    let val = obj.get_property(ctx, vm.intern("x"));
    println!("{}", val.value().as_int32());
}
