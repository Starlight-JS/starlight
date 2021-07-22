/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::prelude::*;
use std::mem::{size_of, ManuallyDrop};

use super::context::Context;
pub struct NumberObject {
    value: f64,
}

extern "C" fn deser(obj: &mut JsObject, deser: &mut Deserializer) {
    *obj.data::<NumberObject>() = ManuallyDrop::new(NumberObject {
        value: f64::from_bits(deser.get_u64()),
    });
}

extern "C" fn ser(obj: &JsObject, serializer: &mut SnapshotSerializer) {
    obj.data::<NumberObject>()
        .get()
        .to_bits()
        .serialize(serializer);
}

extern "C" fn sz() -> usize {
    size_of::<NumberObject>()
}

impl JsClass for NumberObject {
    fn class() -> &'static Class {
        define_jsclass!(
            NumberObject,
            Object,
            None,
            None,
            Some(deser),
            Some(ser),
            Some(sz)
        )
    }
}

impl NumberObject {
    pub fn new(ctx: GcPointer<Context>, value: f64) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(
            ctx,
            &ctx.global_data().number_structure.unwrap(),
            Self::class(),
            ObjectTag::Number,
        );
        *obj.data::<Self>() = ManuallyDrop::new(Self { value });
        obj
    }
    pub fn new_plain(
        ctx: GcPointer<Context>,
        structure: GcPointer<Structure>,
        value: f64,
    ) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(ctx, &structure, Self::class(), ObjectTag::Number);
        *obj.data::<Self>() = ManuallyDrop::new(Self { value });
        obj
    }

    pub fn to_ref(obj: &GcPointer<JsObject>) -> &Self {
        assert!(obj.tag() == ObjectTag::Number);
        obj.data::<Self>()
    }

    pub fn to_mut(obj: &mut GcPointer<JsObject>) -> &mut Self {
        assert!(obj.tag() == ObjectTag::Number);
        obj.data::<Self>()
    }
    #[inline]
    pub fn get(&self) -> f64 {
        self.value
    }
    #[inline]
    pub fn set(&mut self, value: f64) {
        self.value = value;
    }
}
