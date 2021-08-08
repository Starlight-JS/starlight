/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::prelude::*;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    any::TypeId,
    mem::size_of,
};

use super::context::Context;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Variable {
    pub value: JsValue,
    pub mutable: bool,
}

#[repr(C)]
pub struct Environment {
    pub parent: Option<GcPointer<Self>>,
    pub values_ptr: *mut Variable,
    pub values_count: u32,
}

impl Environment {
    pub fn new(mut ctx: GcPointer<Context>, cap: u32) -> GcPointer<Self> {
        unsafe {
            let mut ptr =
                alloc_zeroed(Layout::array::<Variable>(cap as _).unwrap()).cast::<Variable>();

            for i in 0..cap {
                ptr.add(i as _).write(Variable {
                    value: JsValue::encode_undefined_value(),
                    mutable: true,
                });
            }
            ctx.heap().allocate(Self {
                parent: None,
                values_ptr: ptr,
                values_count: cap,
            })
        }
    }

    pub fn as_slice(&self) -> &[Variable] {
        unsafe { std::slice::from_raw_parts(self.values_ptr, self.values_count as _) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [Variable] {
        unsafe { std::slice::from_raw_parts_mut(self.values_ptr, self.values_count as _) }
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.values_ptr.cast(),
                Layout::array::<Variable>(self.values_count as _).unwrap(),
            );
        }
    }
}

impl GcCell for Environment {}

impl Trace for Environment {
    fn trace(&self, visitor: &mut Visitor) {
        self.parent.trace(visitor);
        for var in self.as_slice() {
            var.value.trace(visitor);
        }
    }
}

impl Finalize<Environment> for Environment {}
