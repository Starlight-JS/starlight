/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::structure::Structure;
use crate::gc::{cell::GcPointer, compressed_pointer::CompressedPtr};
use crate::prelude::*;
use std::{any::TypeId, mem::size_of};
pub struct StructureChain {
    pub(crate) vector: Box<[CompressedPtr<Structure>]>,
}

impl StructureChain {
    pub fn head(&self) -> CompressedPtr<Structure> {
        self.vector[0]
    }

    pub fn create(rt: &mut Runtime, head: Option<GcPointer<JsObject>>) -> GcPointer<Self> {
        let mut size = 0;
        let mut current = head;
        while let Some(object) = current {
            size += 1;
            let next = object.structure().get(rt).stored_prototype(rt, &object);
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
            let next = object.structure().get(rt).stored_prototype(rt, &object);
            current = if next.is_jsobject() {
                Some(next.get_jsobject())
            } else {
                None
            };
        }

        rt.heap().allocate(Self {
            vector: buffer.into_boxed_slice(),
        })
    }
}

impl Serializable for StructureChain {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        (self.vector.len() as u32).serialize(serializer);
        for p in self.vector.iter() {
            p.serialize(serializer);
        }
    }
}

impl Deserializable for StructureChain {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let len = u32::deserialize_inplace(deser);
        let mut vec = Vec::with_capacity(len as _);
        for _ in 0..len {
            let val = CompressedPtr::<Structure>::deserialize_inplace(deser);
            vec.push(val);
        }

        Self {
            vector: vec.into_boxed_slice(),
        }
    }

    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }

    unsafe fn allocate(rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl GcCell for StructureChain {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

unsafe impl Trace for StructureChain {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        for structure in self.vector.iter_mut() {
            structure.trace(visitor);
        }
    }
}
