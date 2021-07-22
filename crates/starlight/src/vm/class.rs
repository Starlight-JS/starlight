/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{
    context::Context,
    method_table::MethodTable,
    object::{EnumerationMode, JsHint, JsObject},
    property_descriptor::PropertyDescriptor,
    slot::Slot,
    symbol_table::Symbol,
    value::JsValue,
};
use crate::gc::{
    cell::{GcPointer, Tracer},
    snapshot::{deserializer::Deserializer, serializer::SnapshotSerializer},
};

/// Simple tpe that is used to implement custom JS objects.
pub struct Class {
    /// Class name. `Object.prototype.toString` will print this name.
    pub name: &'static str,
    /// Class method table.
    pub method_table: MethodTable,
    /// `trace` method that is used by GC to mark object.
    pub trace: Option<extern "C" fn(&mut dyn Tracer, &mut JsObject)>,
    pub drop: Option<extern "C" fn(GcPointer<JsObject>)>,
    pub deserialize: Option<extern "C" fn(&mut JsObject, &mut Deserializer)>,
    pub serialize: Option<extern "C" fn(&JsObject, &mut SnapshotSerializer)>,
    pub additional_size: Option<extern "C" fn() -> usize>,
}

/// Define JS class. `$class` is type that will be passed to JS, $name` is class name, and `$sym` is internal class type.
/// There's second macro arm that is used to pass additional methods to class.
#[macro_export]
macro_rules! define_jsclass {
    ($class: ident, $name: ident) => {{
        define_additional_size!($class);
        static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
            name: stringify!($name),
            method_table: js_method_table!($class),
            drop: None,
            trace: None,
            serialize: None,
            deserialize: None,
            additional_size: Some(additional_size),
        };
        &CLASS
    }};
    ($class: ident,$name : ident ,$fin: expr,$trace: expr,$deser: expr,$ser: expr,$size: expr) => {{
        static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
            name: stringify!($name),
            method_table: js_method_table!($class),
            drop: $fin,
            trace: $trace,
            deserialize: $deser,
            serialize: $ser,
            additional_size: $size,
        };
        &CLASS
    }};
}

#[macro_export]
macro_rules! define_additional_size {
    ($class:ident) => {
        extern "C" fn additional_size() -> usize {
            std::mem::size_of::<$class>()
        }
    };
}

#[allow(non_snake_case)]
pub trait JsClass {
    fn class() -> &'static Class;
    fn init(_ctx: GcPointer<Context>) -> Result<(), JsValue> {
        Ok(())
    }
    fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, ctx, collector, mode)
    }
    fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, ctx, hint)
    }
    fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, ctx, index, desc, slot, throwable)
    }
    fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
    fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, ctx, index, val, slot, throwable)
    }
    fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, ctx, name, val, slot, throwable)
    }
    fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, ctx, collector, mode)
    }

    fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, ctx, name, throwable)
    }

    fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, ctx, index, throwable)
    }

    fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, ctx, name, slot)
    }

    fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, ctx, index, slot)
    }
    fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, ctx, name, slot)
    }

    fn DefineOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, ctx, name, desc, slot, throwable)
    }

    fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, ctx, index, slot)
    }
}
