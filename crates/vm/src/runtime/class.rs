use crate::gc::handle::Handle;

use super::{js_object::JsObject, js_string::JsString, method_table::MethodTable, symbol::Symbol};

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
            NOT_CACHED, 38
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
pub struct Class {
    pub name: &'static str,
    pub ty: u32,
    pub method_table: MethodTable,
}

pub struct ClassSlot {
    pub cls: &'static Class,
    pub name: Symbol,
    pub name_string: Handle<JsString>,
    pub constructor: Option<Handle<JsObject>>,
    pub prototype: Option<Handle<JsObject>>,
}

#[macro_export]
macro_rules! define_jsclass_with_symbol {
    ($class: ident,$name : ident,$sym: ident) => {
        pub fn get_class() -> &'static $crate::runtime::class::Class {
            static CLASS: $crate::runtime::class::Class = $crate::runtime::class::Class {
                name: stringify!($name),
                ty: $crate::runtime::class::JsClassType::$sym as _,
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
            };
            &CLASS
        }
    };
}

#[macro_export]
macro_rules! define_jsclass {
    ($class: ident,$name : ident) => {
        define_jsclass_with_symbol!($class, $name, $name);
    };
}
