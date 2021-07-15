/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{method_table::MethodTable, object::JsObject};
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
macro_rules! define_jsclass_with_symbol {
    ($class: ident,$name : ident,$sym: ident) => {
        pub fn get_class() -> &'static $crate::vm::class::Class {
            static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
                name: stringify!($name),
                ty: $crate::vm::class::JsClassType::$sym as _,
                method_table: MethodTable {
                    GetNonIndexedSlot: $class::GetNonIndexedSlotMethod,
                    GetIndexedSlot: $class::GetIndexedSlotMethod,
                    GetNonIndexedPropertySlot: $class::GetNonIndexedPropertySlotMethod,
                    GetIndexedPropertySlot: $class::GetIndexedPropertySlotMethod,
                    GetOwnNonIndexedPropertySlot: $class::GetOwnNonIndexedPropertySlotMethod,
                    GetOwnIndexedPropertySlot: $class::GetOwnIndexedPropertySlotMethod,
                    PutNonIndexedSlot: $class::PutNonIndexedSlotMethod,
                    PutIndexedSlot: $class::PutIndexedSlotMethod,
                    DeleteNonIndexed: $class::DeleteNonIndexedMethod,
                    DeleteIndexed: $class::DeleteIndexedMethod,
                    DefineOwnNonIndexedPropertySlot: $class::DefineOwnNonIndexedPropertySlotMethod,
                    DefineOwnIndexedPropertySlot: $class::DefineOwnIndexedPropertySlotMethod,
                    GetPropertyNames: $class::GetPropertyNamesMethod,
                    GetOwnPropertyNames: $class::GetOwnPropertyNamesMethod,
                    DefaultValue: $class::DefaultValueMethod,
                },
                drop: None,
                trace: None,
                serialize: None,
                deserialize: None,
                additional_size: None,
            };
            &CLASS
        }
    };
    ($class: ident,$name : ident,$sym: ident,$fin: expr,$trace: expr,$deser: expr,$ser: expr,$size: expr) => {
        pub fn get_class() -> &'static $crate::vm::class::Class {
            static CLASS: $crate::vm::class::Class = $crate::vm::class::Class {
                name: stringify!($name),
                ty: $crate::vm::class::JsClassType::$sym as _,
                method_table: MethodTable {
                    GetNonIndexedSlot: $class::GetNonIndexedSlotMethod,
                    GetIndexedSlot: $class::GetIndexedSlotMethod,
                    GetNonIndexedPropertySlot: $class::GetNonIndexedPropertySlotMethod,
                    GetIndexedPropertySlot: $class::GetIndexedPropertySlotMethod,
                    GetOwnNonIndexedPropertySlot: $class::GetOwnNonIndexedPropertySlotMethod,
                    GetOwnIndexedPropertySlot: $class::GetOwnIndexedPropertySlotMethod,
                    PutNonIndexedSlot: $class::PutNonIndexedSlotMethod,
                    PutIndexedSlot: $class::PutIndexedSlotMethod,
                    DeleteNonIndexed: $class::DeleteNonIndexedMethod,
                    DeleteIndexed: $class::DeleteIndexedMethod,
                    DefineOwnNonIndexedPropertySlot: $class::DefineOwnNonIndexedPropertySlotMethod,
                    DefineOwnIndexedPropertySlot: $class::DefineOwnIndexedPropertySlotMethod,
                    GetPropertyNames: $class::GetPropertyNamesMethod,
                    GetOwnPropertyNames: $class::GetOwnPropertyNamesMethod,
                    DefaultValue: $class::DefaultValueMethod,
                },
                drop: $fin,
                trace: $trace,
                deserialize: $deser,
                serialize: $ser,
                additional_size: $size,
            };
            &CLASS
        }
    };
}

/// Same as `define_jsclass_with_symbol!` except `$name` is used as `$symbol`.
#[macro_export]
macro_rules! define_jsclass {
    ($class: ident,$name : ident) => {
        define_jsclass_with_symbol!($class, $name, $name);
    };
}

pub trait JsClass {
    fn class() -> &'static Class;
}
