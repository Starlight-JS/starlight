/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::{collections::HashMap, mem::ManuallyDrop};

use super::{Context, attributes::*, object::{EnumerationMode, JsHint}, property_descriptor::*, slot::*, value::JsValue};
use super::{method_table::*, symbol_table::Internable};
use crate::gc::cell::{GcPointer, Trace, Tracer};
use wtf_rs::segmented_vec::SegmentedVec;

use super::{
    object::{JsObject, ObjectTag},
    property_descriptor::StoredSlot,
    structure::Structure,
    symbol_table::Symbol,
};

pub struct JsGlobal {
    pub(crate) sym_map: HashMap<Symbol, u32>,
    pub(crate) variables: SegmentedVec<StoredSlot>,
    pub(crate) ctx: *mut Context,
}

unsafe impl Trace for JsGlobal {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.variables.iter_mut().for_each(|var| var.trace(visitor));
    }
}

#[allow(non_snake_case)]
impl JsGlobal {
    pub fn new(ctx: &mut Context) -> GcPointer<JsObject> {
        let stack = ctx.shadowstack();
        letroot!(
            shape = stack,
            Structure::new_unique_with_proto(ctx, None, false)
        );
        let js_object = JsObject::new(ctx, &shape, Self::get_class(), ObjectTag::Global);
        {
            *js_object.data::<JsGlobal>() = ManuallyDrop::new(Self {
                sym_map: Default::default(),
                variables: SegmentedVec::with_chunk_size(8),
                ctx: ctx as *mut _,
            });
        }
        js_object
    }
    define_jsclass!(JsGlobal, global);
    pub fn lookup_constant(&self, name: Symbol) -> Option<JsValue> {
        let _ctx = self.ctx;
        if name == "Infinity".intern() {
            Some(JsValue::new(std::f64::INFINITY))
        } else if name == "NaN".intern() {
            Some(JsValue::encode_nan_value())
        } else if name == "undefined".intern() {
            Some(JsValue::encode_undefined_value())
        } else {
            None
        }
    }

    pub fn lookup_variable(&self, name: Symbol) -> Option<u32> {
        self.sym_map.get(&name).copied()
    }
    pub fn push_variable(&mut self, name: Symbol, init: JsValue, attributes: AttrSafe) {
        self.sym_map.insert(name, self.variables.len() as _);
        self.variables.push(StoredSlot::new_raw(init, attributes));
    }

    pub fn point_at(&self, x: u32) -> &StoredSlot {
        &self.variables[x as usize]
    }

    pub fn point_at_mut(&mut self, x: u32) -> &mut StoredSlot {
        &mut self.variables[x as usize]
    }
    pub fn variables(&self) -> &SegmentedVec<StoredSlot> {
        &self.variables
    }

    pub fn variables_mut(&mut self) -> &mut SegmentedVec<StoredSlot> {
        &mut self.variables
    }

    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, ctx, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, ctx, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, ctx, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, ctx, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, ctx, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        for it in obj.as_global().sym_map.iter() {
            collector(*it.0, *it.1);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, ctx, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let entry = obj.as_global().lookup_variable(name);
        if entry.is_some() {
            // all variables are configurable: false
            return Ok(false);
        }
        JsObject::DeleteNonIndexedMethod(obj, ctx, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, ctx, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, ctx, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, ctx, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let global = obj.as_global();

        let entry = global.lookup_variable(name);

        if let Some(entry) = entry {
            let stored = &global.variables[entry as usize];

            slot.set_1(stored.value(), stored.attributes(), Some(obj.as_dyn()));

            return true;
        }

        let res = JsObject::GetOwnNonIndexedPropertySlotMethod(obj, ctx, name, slot);
        if !res {
            slot.make_uncacheable();
        }
        res
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let global = obj.as_global_mut();
        let entry = global.lookup_variable(name);
        if let Some(entry) = entry {
            let mut stored = global.variables[entry as usize];
            let mut returned = false;
            if stored.is_defined_property_accepted(ctx, desc, throwable, &mut returned)? {
                stored.merge(ctx, desc);
                global.variables[entry as usize] = stored;
            }
            return Ok(returned);
        }
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, ctx, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: &mut Context,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
}
