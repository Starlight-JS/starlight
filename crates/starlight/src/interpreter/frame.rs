use crate::{
    bytecode::ByteCode,
    heap::cell::{Cell, Gc, Trace, Tracer},
    runtime::{object::JsObject, value::JsValue},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BlockType {
    Finally,
    Catch,
    Loop,
    Switch,
}

#[repr(C)]

pub struct FrameBase {
    pub prev: *mut Self,

    pub is_bcode: u8,

    pub is_ctor: u8,

    pub is_thrown: u8,
    pub saved_stack: *mut JsValue,
    pub stack_size: usize,
    pub scope: JsValue,

    pub try_stack: Vec<(Gc<JsObject>, *const u8)>,
    pub this_obj: JsValue,
    pub thrown_val: JsValue,
    pub bcode: Option<Gc<ByteCode>>,

    pub code: *mut u8,
    pub callee: JsValue,
}
unsafe impl Trace for FrameBase {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.scope.trace(tracer);
        self.this_obj.trace(tracer);
        self.thrown_val.trace(tracer);
        self.bcode.trace(tracer);
        self.callee.trace(tracer);
        self.try_stack
            .iter()
            .for_each(|(scope, _)| scope.trace(tracer));
    }
}
impl Cell for FrameBase {}
/*
impl Trace for FrameBase {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.bcode.trace(tracer);
        self.callee.trace(tracer);
        self.scope.trace(tracer);
        self.thrown_val.trace(tracer);
        //  self.try_stack.iter().for_each(|val| val.trace(tracer));
        self.this_obj.trace(tracer);
    }
}*/
