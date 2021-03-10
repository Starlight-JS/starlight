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
    let xxx = 42.5f64;
    let regs = starlight::vm::thread::Thread::capture_registers();
    println!("{:?} {}", regs, xxx);
    println!("{}", xxx.to_bits());
}
