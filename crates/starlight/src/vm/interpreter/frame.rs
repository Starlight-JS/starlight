/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::cell::{GcPointer, Trace, Visitor},
    vm::{code_block::CodeBlock, environment::Environment, value::JsValue},
};
use std::mem::size_of;
use wtf_rs::round_up;

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
    pub env: GcPointer<Environment>,
    /// (Environment,Instruction) stack
    pub try_stack: Vec<(Option<GcPointer<Environment>>, *mut u8, *mut JsValue)>,
}
impl CallFrame {
    #[inline(always)]
    pub unsafe fn pop(&mut self) -> JsValue {
        self.sp = self.sp.sub(1);
        self.sp.read()
    }
    pub fn top(&self) -> JsValue {
        unsafe { self.sp.sub(1).read() }
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
impl Trace for CallFrame {
    fn trace(&self, visitor: &mut Visitor) {
        self.callee.trace(visitor);
        self.code_block.trace(visitor);
        self.this.trace(visitor);
        self.env.trace(visitor);
        for (env, _, _) in self.try_stack.iter() {
            env.trace(visitor);
        }
    }
}

pub const FRAME_SIZE: usize =
    round_up(size_of::<CallFrame>(), size_of::<JsValue>()) / size_of::<JsValue>();
