use std::mem::size_of;

use starlight::{
    bytecode::profile::ArithProfile,
    heap::cell::GcPointerBase,
    vm::object::*,
    vm::{
        attributes::string_length,
        interpreter::frame::{CallFrame, FRAME_SIZE},
        structure::Structure,
        symbol_table::Internable,
        value::JsValue,
        Runtime,
    },
    Platform,
};
use wtf_rs::keep_on_stack;

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(false);

    let mut structure = Structure::new_indexed(&mut rt, None, false);
    let transitioned =
        structure.add_property_transition(&mut rt, "x".intern(), string_length(), &mut 0);
    keep_on_stack!(&structure, &transitioned);
    println!("Start GC...");
    rt.heap().gc();
}
