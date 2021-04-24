use crate::{
    gc::cell::{GcCell, GcPointer},
    vm::{environment::Environment, interpreter::frame::CallFrame, value::*, Runtime},
};

pub extern "C" fn type_id_of_object_stub(x: GcPointer<dyn GcCell>) -> u64 {
    unsafe { std::mem::transmute(x.get_dyn().type_id()) }
}
