/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::*;
use crate::gc::cell::Trace;

use std::{intrinsics::unlikely, ptr::null_mut};
#[allow(dead_code)]
pub struct Stack {
    mem: *mut u8,
    start: *mut JsValue,
    pub(crate) cursor: *mut JsValue,
    end: *mut JsValue,
    pub(crate) current: *mut CallFrame,
}

pub const STACK_SIZE: usize = 16 * 1024;

impl Stack {
    pub fn new() -> Self {
        let mut map = unsafe { libc::calloc(1, STACK_SIZE * 8).cast::<u8>() };
        unsafe {
            let mut scan = map.cast::<JsValue>();
            let end = scan.add(STACK_SIZE);
            while scan < end {
                scan.write(JsValue::encode_undefined_value());
                scan = scan.add(1);
            }
        }
        Self {
            start: map.cast(),
            end: unsafe { map.cast::<JsValue>().add(STACK_SIZE) },
            cursor: map.cast(),
            current: null_mut(),
            mem: map.cast(),
        }
    }
    pub fn new_frame(
        &mut self,
        iloc_count: u32,
        _callee: JsValue,
        env: GcPointer<Environment>,
    ) -> Option<*mut CallFrame> {
        unsafe {
            if self.cursor.add(iloc_count as _) >= self.end {
                return None;
            }

            let frame = Box::into_raw(Box::new(CallFrame {
                exit_on_return: true,
                ctor: false,
                prev: self.current,
                try_stack: vec![],
                env,
                this: JsValue::encode_empty_value(),
                sp: self.cursor,
                limit: self.cursor,
                code_block: None,

                callee: JsValue::encode_undefined_value(),
                ip: null_mut(),
            }));
            self.current = frame;

            Some(frame)
        }
    }

    pub fn pop_frame(&mut self) -> Option<Box<CallFrame>> {
        if self.current.is_null() {
            return None;
        }

        unsafe {
            let frame = Box::from_raw(self.current);
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
            //    panic!("Stack underflow");
        }

        unsafe {
            self.cursor = self.cursor.sub(1);
            self.cursor.read()
        }
    }
}

unsafe impl Trace for Stack {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        if !self.current.is_null() {
            unsafe {
                let end = (*self.current).sp;
                let mut scan = self.start;
                while scan < end {
                    (&mut *scan).trace(visitor);
                    scan = scan.add(1);
                }

                let mut frame = self.current;
                while !frame.is_null() {
                    (*frame).trace(visitor);
                    frame = (*frame).prev;
                }
            }
        }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            libc::free(self.mem.cast());
        }
    }
}
