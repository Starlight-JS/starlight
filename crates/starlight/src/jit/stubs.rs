use crate::{
    gc::cell::{GcCell, GcPointer},
    vm::{environment::Environment, interpreter::frame::CallFrame, value::*, Runtime},
};

pub extern "C" fn type_id_of_object_stub(x: GcPointer<dyn GcCell>) -> u64 {
    unsafe { std::mem::transmute(x.get_dyn().type_id()) }
}

pub unsafe extern "C" fn push_env(rt: &mut Runtime, frame: &mut CallFrame) {
    let mut env = Environment::new(rt, 0);
    env.parent = Some(frame.env.unwrap().downcast_unchecked());
    frame.env = Some(env);
}
