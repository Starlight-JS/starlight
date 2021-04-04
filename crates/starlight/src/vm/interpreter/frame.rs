use std::mem::size_of;

use wtf_rs::round_up;

use crate::{
    gc::cell::{GcPointer, Trace, Tracer},
    vm::{code_block::CodeBlock, value::JsValue},
};

#[repr(C, align(8))]
pub struct CallFrame {
    pub prev: *mut CallFrame,
    pub sp: *mut JsValue,
    pub limit: *mut JsValue,
    pub callee: JsValue,
    pub ip: *mut u8,
    pub code_block: Option<GcPointer<CodeBlock>>,
    pub this: JsValue,
    pub ctor: bool,
    pub exit_on_return: bool,
    pub env: JsValue,
    /// (Environment,Instruction) stack
    pub try_stack: Vec<(JsValue, *mut u8, *mut JsValue)>,
    pub locals_start: *mut JsValue,
}
impl CallFrame {
    pub unsafe fn get_iloc(&self, at: u32) -> JsValue {
        self.locals_start.add(at as usize).read()
    }
    pub unsafe fn get_iloc_ptr(&self, at: u32) -> *mut JsValue {
        self.locals_start.add(at as usize)
    }
    pub unsafe fn set_iloc(&mut self, at: u32, val: JsValue) {
        self.locals_start.add(at as usize).write(val);
    }
    #[inline(always)]
    pub unsafe fn pop(&mut self) -> JsValue {
        if self.sp <= self.limit {
            //panic!("stack underflow");
        }
        self.sp = self.sp.sub(1);
        self.sp.read()
    }
    #[inline]
    pub unsafe fn at(&mut self, index: isize) -> &mut JsValue {
        &mut *self.sp.offset(index)
    }
    #[inline(always)]
    pub unsafe fn push(&mut self, val: JsValue) {
        self.sp.write(val);
        self.sp = self.sp.add(1);
    }
}
unsafe impl Trace for CallFrame {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.callee.trace(visitor);
        self.code_block.trace(visitor);
        self.this.trace(visitor);
        self.env.trace(visitor);
        for (env, _, _) in self.try_stack.iter_mut() {
            env.trace(visitor);
        }
    }
}

pub const FRAME_SIZE: usize =
    round_up(size_of::<CallFrame>(), size_of::<JsValue>()) / size_of::<JsValue>();
