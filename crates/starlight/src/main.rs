use starlight::{
    bytecode::profile::ArithProfile,
    heap::cell::GcPointerBase,
    vm::{value::JsValue, Runtime},
    Platform,
};

fn main() {
    Platform::initialize();
    let _rt = Runtime::new(false);
    let base = GcPointerBase::new(42);
    println!("{}", base.vtable());
    drop(_rt);
}
