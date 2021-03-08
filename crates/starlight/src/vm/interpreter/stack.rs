use std::{intrinsics::unlikely, ptr::null_mut};

use super::*;
use crate::vm::{string::JsString, value::*};
use crate::{
    heap::{cell::Trace, SlotVisitor},
    vm::error::*,
};
use frame::*;
use memmap2::MmapMut;
pub struct Stack {
    map: MmapMut,
    start: *mut JsValue,
    cursor: *mut JsValue,
    size: usize,
    end: *mut JsValue,
    current: *mut CallFrame,
}

pub const STACK_SIZE: usize = 16 * 1024;

impl Stack {
    pub fn new() -> Self {
        let mut map =
            MmapMut::map_anon(STACK_SIZE * 8).expect("Failed to allocate interpreter stack memory");
        Self {
            start: map.as_mut_ptr().cast(),
            end: unsafe { map.as_mut_ptr().cast::<JsValue>().add(STACK_SIZE) },
            cursor: map.as_mut_ptr().cast(),
            current: null_mut(),
            size: STACK_SIZE,
            map,
        }
    }
    pub fn new_frame(&mut self) -> Option<*mut CallFrame> {
        unsafe {
            if self.cursor.add(FRAME_SIZE) >= self.end {
                return None;
            }

            let value = self.cursor.cast::<CallFrame>();
            self.cursor = self.cursor.add(FRAME_SIZE);
            value.write(CallFrame {
                exit_on_return: false,
                ctor: false,
                prev: self.current,
                try_stack: vec![],
                env: JsValue::encode_empty_value(),
                this: JsValue::encode_empty_value(),
                sp: self.cursor,
                code_block: None,
                callee: JsValue::encode_undefined_value(),
                ip: null_mut(),
            });
            self.current = value;
            Some(value)
        }
    }

    pub fn pop_frame(&mut self) -> Option<CallFrame> {
        if self.current.is_null() {
            return None;
        }

        unsafe {
            let frame = self.current.read();
            self.current = frame.prev;
            self.cursor = if frame.prev.is_null() {
                self.start
            } else {
                (*frame.prev).sp
            };
            Some(frame)
        }
    }
    #[inline]
    pub fn push(&mut self, val: JsValue) {
        if unlikely(self.cursor == self.end) {
            panic!("stack overflow");
        }
        unsafe {
            self.cursor.write(val);
            self.cursor = self.cursor.add(1);
        }
    }

    #[inline]
    pub fn pop(&mut self) -> JsValue {
        if unlikely(self.cursor == self.start) {
            panic!("Stack underflow");
        }

        unsafe {
            self.cursor = self.cursor.sub(1);
            self.cursor.read()
        }
    }
}

unsafe impl Trace for Stack {
    fn trace(&self, visitor: &mut SlotVisitor) {
        if !self.current.is_null() {
            unsafe {
                visitor.add_conservative_roots(self.start as _, (*self.current).sp as _);
                let mut frame = self.current;
                while !frame.is_null() {
                    (*frame).trace(visitor);
                    frame = (*frame).prev;
                }
            }
        }
    }
}
