use crate::{
    bytecode::ByteCode,
    heap::cell::{Cell, Gc, Trace, Tracer},
    runtime::value::JsValue,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BlockType {
    Finally,
    Catch,
    Loop,
    Switch,
}

use starlight_derive::Trace;
#[repr(C)]
#[derive(Trace)]
pub struct FrameBase {
    #[unsafe_ignore_trace]
    pub prev: *mut Self,
    #[unsafe_ignore_trace]
    pub is_bcode: u8,
    #[unsafe_ignore_trace]
    pub is_ctor: u8,
    #[unsafe_ignore_trace]
    pub is_thrown: u8,
    #[unsafe_ignore_trace]
    pub stack_size: usize,
    pub scope: JsValue,
    #[unsafe_ignore_trace]
    pub try_stack: Vec<*const u8>,
    pub this_obj: JsValue,
    pub thrown_val: JsValue,
    pub bcode: Option<Gc<ByteCode>>,
    #[unsafe_ignore_trace]
    pub code: *mut u8,
    pub callee: JsValue,
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
