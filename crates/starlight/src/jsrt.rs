use crate::{
    runtime::{arguments::Arguments, function::JsNativeFunction, value::JsValue},
    vm::VirtualMachine,
};
pub mod array;
pub mod error;
pub mod object;
pub fn print(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    for ix in 0..args.size() {
        let val = args.at(ix);
        let s = val.to_string(vm)?;
        print!("{}", s);
    }
    println!();

    Ok(JsValue::undefined())
}

pub fn jsrt_init(vm: &mut VirtualMachine) {
    vm.space().defer_gc();
    let mut global = vm.global_object();
    let name = vm.intern("print");
    let print = JsNativeFunction::new(vm, name, print, 0);
    assert!(global.put(vm, name, JsValue::new(print), false).is_ok());

    vm.space().undefer_gc();
}
