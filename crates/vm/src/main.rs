use starlight_vm::runtime::{
    js_object::{JsObject, ObjectTag},
    js_value::JsValue,
    structure::Structure,
    vm::JsVirtualMachine,
};
use starlight_vm::runtime::{options::Options, symbol::Symbol};

use wtf_rs::keep_on_stack;

fn main() {
    let mut vm = JsVirtualMachine::create(Options {
        verbose_gc: true,
        ..Default::default()
    });
    {
        let _ctx = vm.make_context();
        let my_struct = Structure::new_(&mut vm, &[]);
        let mut obj = JsObject::new(&mut vm, my_struct, JsObject::get_class(), ObjectTag::Array);
        keep_on_stack!(&obj, &my_struct);
        let _ = obj.put(&mut vm, Symbol::Indexed(4), JsValue::new_int(42), false);
        vm.gc(false);
        let val = obj.get_property(&mut vm, Symbol::Indexed(4));
        // assert!(val.is_data());
        assert!(val.value().is_int32());
        assert_eq!(val.value().as_int32(), 42);

        drop(vm);
    }
}
