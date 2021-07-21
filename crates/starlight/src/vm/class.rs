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

#[macro_export]
macro_rules! js_class_list {
    ($f: ident) => {
        $f! {
            Object, 0,
            Function, 1,
            Array, 2,
            Date, 3,
            String, 4,
            Boolean, 5,
            Number, 6,
            RegExp, 7,
            Math, 8,
            JSON, 9,
            Error, 10,
            EvalError, 11,
            RangeError, 12,
            ReferenceError, 13,
            SyntaxError, 14,
            TypeError, 15,
            URIError, 16,
            global, 17,
            Arguments, 18,
            Set, 19,
            /* i18n */
            Collator, 20,
            NumberFormat, 21,
            DateTimeFormat, 22,
            Name, 23,
            /* iterator */
            Iterator, 24,
            /* Binary Blocks */
            ArrayBuffer, 25,
            DataView, 26,
            Int8Array, 27,
            Uint8Array, 28,
            Int16Array, 29,
            Uint16Array, 30,
            Int32Array, 31,
            Uint32Array, 32,
            Float32Array, 33,
            Float64Array, 34,
            Uint8ClampedArray, 35,
            Reflect, 36,
            Symbol, 37,
            NOT_CACHED, 38,
            NUM_OF_CLASS,39
        }
    };
}

macro_rules! def_enum {
    ($($name : ident,$num: expr),*) => {
        #[allow(non_camel_case_types)]
        #[derive(Copy,Clone,PartialEq,Eq,Hash,Debug)]
        #[repr(u8)]
        pub enum JsClassType {
            $($name = $num),*
        }
    };
}

js_class_list!(def_enum);

/// Simple tpe that is used to implement custom JS objects.
pub struct Class {
    /// Class name. `Object.prototype.toString` will print this name.
    pub name: &'static str,
    /// Internal class type.
    pub ty: u32,
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
    ($class: ident,$sym: ident) => {
        impl JsClassMethodTable for $class {}
        impl $class {
            pub fn get_class() -> &'static $crate::vm::class::Class {
                static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
                    name: stringify!($sym),
                    ty: $crate::vm::class::JsClassType::$sym as _,
                    method_table: js_method_table!($class),
                    drop: None,
                    trace: None,
                    serialize: None,
                    deserialize: None,
                    additional_size: None,
                };
                &CLASS
            }
        }
    };
    ($class: ident, $name: ident ,$sym: ident) => {
        impl JsClassMethodTable for $class {}
        impl $class {
            pub fn get_class() -> &'static $crate::vm::class::Class {
                static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
                    name: stringify!($name),
                    ty: $crate::vm::class::JsClassType::$sym as _,
                    method_table: js_method_table!($class),
                    drop: None,
                    trace: None,
                    serialize: None,
                    deserialize: None,
                    additional_size: None,
                };
                &CLASS
            }
        }
    };
    ($class: ident,$name : ident,$sym: ident,$fin: expr,$trace: expr,$deser: expr,$ser: expr,$size: expr) => {
        impl JsClassMethodTable for $class {}
        impl $class {
            pub fn get_class() -> &'static $crate::vm::class::Class {
                static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
                    name: stringify!($name),
                    ty: $crate::vm::class::JsClassType::$sym as _,
                    method_table: js_method_table!($class),
                    drop: $fin,
                    trace: $trace,
                    deserialize: $deser,
                    serialize: $ser,
                    additional_size: $size,
                };
                &CLASS
            }
        }
    };
}

/// Same as `define_jsclass_with_symbol!` except `$name` is used as `$symbol`.

pub trait JsClass {
    fn class() -> &'static Class;
}

#[allow(non_snake_case)]
pub trait JsClassMethodTable {
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
