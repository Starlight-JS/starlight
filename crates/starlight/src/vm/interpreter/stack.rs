use super::*;
use crate::heap::cell::Trace;
use frame::*;
use memmap2::MmapMut;
use std::{intrinsics::unlikely, ptr::null_mut};
#[allow(dead_code)]
pub struct Stack {
    map: MmapMut,
    start: *mut JsValue,
    pub(crate) cursor: *mut JsValue,
    end: *mut JsValue,
    pub(crate) current: *mut CallFrame,
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
            map,
        }
    }
    pub fn new_frame(&mut self) -> Option<*mut CallFrame> {
        unsafe {
            if self.cursor.add(FRAME_SIZE) >= self.end {
                return None;
            }

            let frame = Box::into_raw(Box::new(CallFrame {
                exit_on_return: true,
                ctor: false,
                prev: self.current,
                try_stack: vec![],
                env: JsValue::encode_empty_value(),
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
                (*frame.prev).limit
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
