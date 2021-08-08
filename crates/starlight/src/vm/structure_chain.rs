/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{structure::Structure, Context};
use crate::gc::cell::GcPointer;
use crate::prelude::*;

pub struct StructureChain {
    pub(crate) vector: Box<[GcPointer<Structure>]>,
}
impl Finalize<StructureChain> for StructureChain {}
impl StructureChain {
    pub fn head(&self) -> GcPointer<Structure> {
        self.vector[0]
    }

    pub fn create(
        mut ctx: GcPointer<Context>,
        head: Option<GcPointer<JsObject>>,
    ) -> GcPointer<Self> {
        let mut size = 0;
        let mut current = head;
        while let Some(object) = current {
            size += 1;
            let next = object.structure().stored_prototype(ctx, &object);
            current = if next.is_jsobject() {
                Some(next.get_jsobject())
            } else {
                None
            };
        }
        let mut buffer = Vec::with_capacity(size);

        let mut current = head;
        while let Some(object) = current {
            let structure = object.structure();
            buffer.push(structure);
            let next = object.structure().stored_prototype(ctx, &object);
            current = if next.is_jsobject() {
                Some(next.get_jsobject())
            } else {
                None
            };
        }

        ctx.heap().allocate(Self {
            vector: buffer.into_boxed_slice(),
        })
    }
}

impl GcCell for StructureChain {}

impl Trace for StructureChain {
    fn trace(&self, visitor: &mut Visitor) {
        for structure in self.vector.iter() {
            structure.trace(visitor);
        }
    }
}
