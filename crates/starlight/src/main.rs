use std::mem::size_of;

use starlight::{
    bytecode::profile::ArithProfile,
    heap::cell::GcPointerBase,
    vm::object::*,
    vm::{
        interpreter::frame::{CallFrame, FRAME_SIZE},
        value::JsValue,
        Runtime,
    },
    Platform,
};

fn main() {
    Platform::initialize();
    println!("{} {}", FRAME_SIZE, size_of::<CallFrame>() / 8);
}
