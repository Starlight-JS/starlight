use crate::heap::context::LocalContext;
use crate::{
    runtime::{arguments::Arguments, function::JsNativeFunction, value::JsValue},
    vm::VirtualMachine,
};
pub mod array;
pub mod error;
pub fn print(
    vm: &mut VirtualMachine,
    ctx: &LocalContext<'_>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    for ix in 0..args.size() {
        let val = args[ix];
        let s = val.to_string(vm)?;
        print!("{}", s);
    }
    println!();

    Ok(JsValue::undefined())
}

pub fn jsrt_init(vm: &mut VirtualMachine) {
    let ctx = vm.space().new_local_context();
    let mut global = ctx.new_local(vm.global_object());
    let name = vm.intern("print");
    let print = ctx.new_local(JsNativeFunction::new(vm, name, print, 0));
    assert!(global.put(vm, name, JsValue::new(*print), false).is_ok());
}
