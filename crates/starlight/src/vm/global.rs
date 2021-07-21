/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::prelude::*;
use std::{collections::HashMap, mem::ManuallyDrop};

use super::{
    attributes::*,
    object::{EnumerationMode},
    property_descriptor::*,
    slot::*,
    value::JsValue,
    Context,
};
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
    pub(crate) ctx: GcPointer<Context>,
}

unsafe impl Trace for JsGlobal {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.variables.iter_mut().for_each(|var| var.trace(visitor));
    }
}

define_jsclass!(JsGlobal, global);

#[allow(non_snake_case)]
impl JsGlobal {
    pub fn new(mut ctx: GcPointer<Context>) -> GcPointer<JsObject> {
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
                ctx,
            });
        }
        js_object
    }

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

    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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
        ctx: GcPointer<Context>,
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
    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
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
}
