/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::{
        cell::*,
        snapshot::{
            deserializer::Deserializer,
            serializer::{Serializable, SnapshotSerializer},
        },
    },
    jsrt::boolean::BooleanObject,
    vm::interpreter::SpreadValue,
};

use std::{
    any::TypeId,
    convert::TryFrom,
    hash::{Hash, Hasher},
    hint::unreachable_unchecked,
    intrinsics::{likely, size_of, unlikely},
};

use super::{
    attributes::*,
    class::JsClass,
    error::*,
    number::*,
    object::{JsHint, JsObject, TypedJsObject},
    slot::*,
    string::*,
    symbol_table::*,
    Context, VirtualMachine,
};
pub const CMP_FALSE: i32 = 0;
pub const CMP_TRUE: i32 = 1;
pub const CMP_UNDEF: i32 = -1;

#[cfg(target_pointer_width = "64")]
pub use new_value::*;
#[cfg(target_pointer_width = "32")]
pub use old_value::*;
pub mod old_value {
    use super::*;
    pub type TagKind = u32;

    pub const FIRST_TAG: TagKind = 0xfff9;
    pub const LAST_TAG: TagKind = 0xffff;
    pub const EMPTY_INVALID_TAG: u32 = FIRST_TAG;
    pub const UNDEFINED_NULL_TAG: u32 = FIRST_TAG + 1;
    pub const BOOL_TAG: u32 = FIRST_TAG + 2;
    pub const INT32_TAG: u32 = FIRST_TAG + 3;
    pub const NATIVE_VALUE_TAG: u32 = FIRST_TAG + 4;
    pub const STR_TAG: u32 = FIRST_TAG + 5;
    pub const OBJECT_TAG: u32 = FIRST_TAG + 6;
    pub const FIRST_PTR_TAG: u32 = STR_TAG;

    #[repr(u32)]
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ExtendedTag {
        Empty = EMPTY_INVALID_TAG * 2 + 1,
        Undefined = UNDEFINED_NULL_TAG * 2,
        Null = UNDEFINED_NULL_TAG * 2 + 1,
        Bool = BOOL_TAG * 2,
        Int32 = INT32_TAG * 2,
        Native1 = NATIVE_VALUE_TAG * 2,
        Native2 = NATIVE_VALUE_TAG * 2 + 1,
        Str1 = STR_TAG * 2,
        Str2 = STR_TAG * 2 + 1,
        Object1 = OBJECT_TAG * 2,
        Object2 = OBJECT_TAG * 2 + 1,
    }

    /// A NaN-boxed encoded value.
    #[derive(Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct JsValue(u64);

    impl JsValue {
        pub const NUM_TAG_EXP_BITS: u32 = 16;
        pub const NUM_DATA_BITS: u32 = (64 - Self::NUM_TAG_EXP_BITS);
        pub const TAG_WIDTH: u32 = 4;
        pub const TAG_MASK: u32 = (1 << Self::TAG_WIDTH) - 1;
        pub const DATA_MASK: u64 = (1 << Self::NUM_DATA_BITS as u64) - 1;
        pub const ETAG_WIDTH: u32 = 5;
        pub const ETAG_MASK: u32 = (1 << Self::ETAG_WIDTH) - 1;
        #[inline]
        pub const fn from_raw(x: u64) -> Self {
            Self(x)
        }
        #[inline]
        pub const fn get_tag(&self) -> TagKind {
            (self.0 >> Self::NUM_DATA_BITS as u64) as u32
        }
        #[inline]
        pub fn get_etag(&self) -> ExtendedTag {
            unsafe { std::mem::transmute((self.0 >> (Self::NUM_DATA_BITS as u64 - 1)) as u32) }
        }
        #[inline]
        pub const fn combine_tags(a: TagKind, b: TagKind) -> u32 {
            ((a & Self::TAG_MASK) << Self::TAG_WIDTH) | (b & Self::TAG_MASK)
        }
        #[inline]
        const fn internal_new(val: u64, tag: TagKind) -> Self {
            Self(val | ((tag as u64) << Self::NUM_DATA_BITS))
        }
        #[inline]
        const fn new_extended(val: u64, tag: ExtendedTag) -> Self {
            Self(val | ((tag as u64) << (Self::NUM_DATA_BITS - 1)))
        }
        #[inline]
        pub const fn encode_null_ptr_object_value() -> Self {
            Self::internal_new(0, OBJECT_TAG)
        }
        #[inline]
        pub fn encode_object_value<T: GcCell + ?Sized>(val: GcPointer<T>) -> Self {
            Self::internal_new(
                unsafe { std::mem::transmute::<_, usize>(val) } as _,
                OBJECT_TAG,
            )
        }
        #[inline]
        pub const fn encode_native_u32(val: u32) -> Self {
            Self::internal_new(val as _, NATIVE_VALUE_TAG)
        }
        #[inline]
        pub fn encode_native_pointer(p: *const ()) -> Self {
            Self::internal_new(p as _, NATIVE_VALUE_TAG)
        }
        #[inline]
        pub const fn encode_bool_value(val: bool) -> Self {
            Self::internal_new(val as _, BOOL_TAG)
        }
        #[inline]
        pub const fn encode_null_value() -> Self {
            Self::new_extended(0, ExtendedTag::Null)
        }
        #[inline]
        pub fn encode_int32(x: i32) -> Self {
            Self::internal_new(x as u32 as u64, INT32_TAG)
        }
        #[inline]
        pub const fn encode_undefined_value() -> Self {
            Self::new_extended(0, ExtendedTag::Undefined)
        }
        #[inline]
        pub const fn encode_empty_value() -> Self {
            Self::new_extended(0, ExtendedTag::Empty)
        }
        #[inline]
        pub fn encode_f64_value(x: f64) -> Self {
            Self::from_raw(x.to_bits())
        }

        #[inline]
        pub const fn encode_nan_value() -> Self {
            Self::from_raw(0x7ff8000000000000)
        }
        #[inline]
        pub fn encode_untrusted_f64_value(val: f64) -> Self {
            if val.is_nan() {
                return Self::encode_nan_value();
            }
            Self::encode_f64_value(val)
        }

        #[inline]
        pub fn update_pointer(&self, val: *const ()) -> Self {
            Self::internal_new(val as _, self.get_tag())
        }

        #[inline]
        pub unsafe fn unsafe_update_pointer(&mut self, val: *const ()) {
            self.0 = val as u64 | (self.get_tag() as u64) << Self::NUM_DATA_BITS as u64
        }

        #[inline]
        pub fn is_null(&self) -> bool {
            self.get_etag() == ExtendedTag::Null
        }
        #[inline]
        pub fn is_undefined(&self) -> bool {
            self.get_etag() == ExtendedTag::Undefined
        }

        #[inline]
        pub fn is_empty(&self) -> bool {
            self.get_etag() == ExtendedTag::Empty
        }

        #[inline]
        pub fn is_native_value(&self) -> bool {
            self.get_tag() == NATIVE_VALUE_TAG
        }

        #[inline]
        pub fn is_int32(&self) -> bool {
            self.get_tag() == INT32_TAG
        }

        #[inline]
        pub fn is_bool(&self) -> bool {
            self.get_tag() == BOOL_TAG
        }

        #[inline]
        pub fn is_object(&self) -> bool {
            self.get_tag() == OBJECT_TAG
        }

        #[inline]
        pub fn is_double(&self) -> bool {
            self.0 < ((FIRST_TAG as u64) << Self::NUM_DATA_BITS as u64)
        }

        #[inline]
        pub fn is_pointer(&self) -> bool {
            self.0 >= ((FIRST_PTR_TAG as u64) << Self::NUM_DATA_BITS as u64)
        }

        #[inline]
        pub fn get_raw(&self) -> u64 {
            self.0
        }

        #[inline]
        pub fn get_pointer(&self) -> *mut () {
            assert!(self.is_pointer());
            unsafe { std::mem::transmute((self.0 & Self::DATA_MASK) as usize) }
        }
        #[inline]
        pub fn get_int32(&self) -> i32 {
            assert!(self.is_int32());
            self.0 as u32 as i32
        }
        #[inline]
        pub fn get_double(&self) -> f64 {
            f64::from_bits(self.0)
        }
        #[inline]
        pub fn get_native_value(&self) -> i64 {
            assert!(self.is_native_value());
            (((self.0 & Self::DATA_MASK as u64) as i64) << (64 - Self::NUM_DATA_BITS as i64))
                >> (64 - Self::NUM_DATA_BITS as i64)
        }

        #[inline]
        pub fn get_native_u32(&self) -> u32 {
            assert!(self.is_native_value());
            self.0 as u32
        }

        #[inline]
        pub fn get_native_ptr(&self) -> *mut () {
            assert!(self.is_native_value());
            (self.0 & Self::DATA_MASK) as *mut ()
        }

        #[inline]
        pub fn get_bool(&self) -> bool {
            assert!(self.is_bool());
            (self.0 & 0x1) != 0
        }

        #[inline]
        pub fn get_object(&self) -> GcPointer<dyn GcCell> {
            assert!(self.is_object());
            unsafe {
                std::mem::transmute::<_, GcPointer<dyn GcCell>>((self.0 & Self::DATA_MASK) as usize)
            }
        }

        /// Get number value from JS value.If value is int32 value then it is casted to f64.
        #[inline]
        pub fn get_number(&self) -> f64 {
            if self.is_int32() {
                return self.get_int32() as f64;
            }
            self.get_double()
        }

        pub unsafe fn set_no_barrier(&mut self, val: Self) {
            self.0 = val.0;
        }

        pub fn is_number(&self) -> bool {
            self.is_double() || self.is_int32()
        }
    }
}

unsafe impl Trace for JsValue {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        if self.is_object() {
            visitor.visit(self.get_object());
        }
    }
}

impl JsValue {
    /// This is a more specialized version of `to_numeric`, including `BigInt`.
    ///
    /// This function is equivalent to `Number(value)` in JavaScript
    ///
    /// See: <https://tc39.es/ecma262/#sec-tonumeric>
    pub fn to_numeric_number(self, ctx: GcPointer<Context>) -> Result<f64, JsValue> {
        let primitive = self.to_primitive(ctx, JsHint::Number)?;
        primitive.to_number(ctx)
    }
    #[inline]
    pub unsafe fn fill(start: *mut Self, end: *mut Self, fill: JsValue) {
        let mut cur = start;
        while cur != end {
            cur.write(fill);
            cur = cur.add(1);
        }
    }

    #[inline]
    pub unsafe fn uninit_copy(
        mut first: *mut Self,
        last: *mut Self,
        mut result: *mut JsValue,
    ) -> *mut JsValue {
        while first != last {
            result.write(first.read());
            first = first.add(1);
            result = result.add(1);
        }
        result
    }

    #[inline]
    pub unsafe fn copy_backward(
        first: *mut Self,
        mut last: *mut Self,
        mut result: *mut JsValue,
    ) -> *mut JsValue {
        while first != last {
            last = last.sub(1);
            result = result.sub(1);
            result.write(last.read());
        }
        result
    }
    #[inline]
    pub unsafe fn copy(
        mut first: *mut Self,
        last: *mut Self,
        mut result: *mut JsValue,
    ) -> *mut JsValue {
        while first != last {
            result.write(first.read());
            first = first.add(1);
            result = result.add(1);
        }
        result
    }

    pub fn same_value_impl(lhs: Self, rhs: Self, _zero: bool) -> bool {
        if lhs.is_number() {
            if !rhs.is_number() {
                return false;
            }

            let lhsn = lhs.get_number();
            let rhsn = rhs.get_number();
            if lhsn == rhsn {
                return true;
            }
            return lhsn.is_nan() && rhsn.is_nan();
        }

        if !lhs.is_object() || !rhs.is_object() {
            return lhs.get_raw() == rhs.get_raw();
        }
        if lhs.is_object()
            && rhs.is_object()
            && lhs.get_object().is::<JsString>()
            && rhs.get_object().is::<JsString>()
        {
            return unsafe {
                lhs.get_object().downcast_unchecked::<JsString>().as_str()
                    == rhs.get_object().downcast_unchecked::<JsString>().as_str()
            };
        }
        lhs.get_raw() == rhs.get_raw()
    }
    pub fn same_value(x: JsValue, y: JsValue) -> bool {
        Self::same_value_impl(x, y, false)
    }
    pub fn same_value_zero(lhs: Self, rhs: Self) -> bool {
        Self::same_value_impl(lhs, rhs, true)
    }
    pub fn to_object(self, ctx: GcPointer<Context>) -> Result<GcPointer<JsObject>, JsValue> {
        if self.is_undefined() || self.is_null() {
            let msg = JsString::new(ctx, "ToObject to null or undefined");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }
        if self.is_object() && self.get_object().is::<JsObject>() {
            return Ok(unsafe { self.get_object().downcast_unchecked() });
        }
        if self.is_number() {
            return Ok(NumberObject::new(ctx, self.get_number()));
        }
        if self.is_jsstring() {
            return Ok(JsStringObject::new(ctx, self.get_jsstring()));
        }
        if self.is_symbol() {
            return Ok(JsSymbolObject::new(ctx, unsafe {
                self.get_object().downcast_unchecked()
            }));
        }
        if self.is_bool() {
            return Ok(BooleanObject::new(ctx, self.get_bool()));
        }
        Err(JsValue::new(
            ctx.new_type_error("NYI: JsValue::to_object cases"),
        ))
    }
    pub fn is_jsobject(self) -> bool {
        self.is_object() && self.get_object().is::<JsObject>()
    }
    pub fn is_symbol(self) -> bool {
        self.is_object() && self.get_object().is::<JsSymbol>()
    }
    pub fn to_primitive(self, ctx: GcPointer<Context>, hint: JsHint) -> Result<JsValue, JsValue> {
        if self.is_object() && self.get_object().is::<JsObject>() {
            let mut object = unsafe { self.get_object().downcast_unchecked::<JsObject>() };
            object.to_primitive(ctx, hint)
        } else {
            Ok(self)
        }
    }
    fn number_compare(x: f64, y: f64) -> i32 {
        if x.is_nan() || y.is_nan() {
            return CMP_UNDEF;
        }
        if x == y {
            return CMP_FALSE;
        }
        if x < y {
            CMP_TRUE
        } else {
            CMP_FALSE
        }
    }
    pub fn get_string(&self) -> GcPointer<JsString> {
        assert!(self.is_jsstring());
        unsafe { self.get_object().downcast_unchecked() }
    }
    pub fn is_jsstring(self) -> bool {
        self.is_object() && self.get_object().is::<JsString>()
    }
    pub fn is_string(self) -> bool {
        self.is_jsstring()
    }

    pub fn abstract_equal(self, other: JsValue, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
        let mut lhs = self;
        let mut rhs = other;

        loop {
            if likely(lhs.is_int32() && rhs.is_int32()) {
                return Ok(lhs.get_int32() == rhs.get_int32());
            }
            if likely(lhs.is_number() && rhs.is_number()) {
                return Ok(lhs.get_number() == rhs.get_number());
            }

            if (lhs.is_undefined() || lhs.is_null()) && (rhs.is_undefined() || rhs.is_null()) {
                return Ok(true);
            }

            if lhs.is_jsstring() && rhs.is_jsstring() {
                return Ok(lhs.get_string().as_str() == rhs.get_string().as_str());
            }

            if lhs.is_symbol() && rhs.is_symbol() {
                return Ok(lhs.get_raw() == rhs.get_raw());
            }
            if lhs.is_jsobject() && rhs.is_jsobject() {
                return Ok(lhs.get_raw() == rhs.get_raw());
            }
            if lhs.is_number() && rhs.is_jsstring() {
                rhs = JsValue::new(rhs.to_number(ctx)?);
                continue;
            }
            if lhs.is_jsstring() && rhs.is_number() {
                lhs = JsValue::new(lhs.to_number(ctx)?);
                continue;
            }

            if lhs.is_bool() {
                lhs = JsValue::new(lhs.to_number(ctx)?);
                continue;
            }

            if rhs.is_bool() {
                rhs = JsValue::new(rhs.to_number(ctx)?);
                continue;
            }

            if (lhs.is_jsstring() || lhs.is_number()) && rhs.is_object() {
                rhs = rhs.to_primitive(ctx, JsHint::None)?;
                continue;
            }

            if lhs.is_object() && (rhs.is_jsstring() || rhs.is_number()) {
                lhs = lhs.to_primitive(ctx, JsHint::None)?;
                continue;
            }
            break Ok(false);
        }
    }

    pub fn strict_equal(self, other: JsValue) -> bool {
        if likely(self.is_number() && other.is_number()) {
            return self.get_number() == other.get_number();
        }

        if !self.is_object() || !other.is_object() {
            return self.get_raw() == other.get_raw();
        }

        if self.is_jsstring() && other.is_jsstring() {
            return self.get_string().as_str() == other.get_string().as_str();
        }
        self.get_raw() == other.get_raw()
    }
    #[inline]
    pub fn compare(
        self,
        rhs: Self,
        left_first: bool,
        ctx: GcPointer<Context>,
    ) -> Result<i32, JsValue> {
        let lhs = self;
        if likely(lhs.is_number() && rhs.is_number()) {
            return Ok(Self::number_compare(lhs.get_number(), rhs.get_number()));
        }

        let px;
        let py;
        if left_first {
            px = lhs.to_primitive(ctx, JsHint::Number)?;
            py = rhs.to_primitive(ctx, JsHint::Number)?;
        } else {
            py = rhs.to_primitive(ctx, JsHint::Number)?;
            px = lhs.to_primitive(ctx, JsHint::Number)?;
        }
        if likely(px.is_number() && py.is_number()) {
            return Ok(Self::number_compare(px.get_number(), py.get_number()));
        }
        if likely(px.is_jsstring() && py.is_jsstring()) {
            #[inline(never)]
            fn slow_string_cmp(x: &str, y: &str) -> Result<i32, JsValue> {
                if x.starts_with(y) {
                    return Ok(CMP_FALSE);
                }
                if y.starts_with(x) {
                    return Ok(CMP_TRUE);
                }
                for (x, y) in x.chars().zip(y.chars()) {
                    if x != y {
                        return Ok(if x < y { CMP_TRUE } else { CMP_FALSE });
                    }
                }
                unreachable!()
            }
            let x = px.get_string();
            let y = py.get_string();
            let (x, y) = (x.as_str(), y.as_str());
            return slow_string_cmp(x, y);
        } else {
            let nx = px.to_number(ctx)?;
            let ny = py.to_number(ctx)?;
            Ok(Self::number_compare(nx, ny))
        }
    }
    pub fn compare_left(self, rhs: Self, ctx: GcPointer<Context>) -> Result<i32, JsValue> {
        Self::compare(self, rhs, true, ctx)
    }
    pub fn to_int32(self, ctx: GcPointer<Context>) -> Result<i32, JsValue> {
        if self.is_int32() {
            return Ok(self.get_int32());
        }
        let number = self.to_number(ctx)?;
        if unlikely(number.is_nan() || number.is_infinite()) {
            return Ok(0);
        }
        Ok(number.floor() as i32)
    }

    pub fn to_uint32(self, ctx: GcPointer<Context>) -> Result<u32, JsValue> {
        if self.is_int32() {
            return Ok(self.get_int32() as _);
        }
        let number = self.to_number(ctx)?;
        if unlikely(number.is_nan() || number.is_infinite()) {
            return Ok(0);
        }
        Ok(number.floor() as u32)
    }

    pub fn to_number(self, ctx: GcPointer<Context>) -> Result<f64, JsValue> {
        if likely(self.is_double()) {
            Ok(self.get_double())
        } else if likely(self.is_int32()) {
            Ok(self.get_int32() as _)
        } else if self.is_object() && self.get_object().is::<JsString>() {
            let s = unsafe { self.get_object().downcast_unchecked::<JsString>() };
            if let Ok(n) = s.as_str().parse::<i32>() {
                return Ok(n as f64);
            }
            Ok(s.as_str()
                .parse::<f64>()
                .unwrap_or_else(|_| f64::from_bits(0x7ff8000000000000)))
        } else if self.is_bool() {
            Ok(self.get_bool() as u8 as f64)
        } else if self.is_null() {
            Ok(0.0)
        } else if self.is_undefined() {
            Ok(f64::from_bits(0x7ff8000000000000))
        } else if self.is_object() && self.get_object().is::<JsObject>() {
            let stack = ctx.shadowstack();
            letroot!(obj = stack, unsafe {
                self.get_object().downcast_unchecked::<JsObject>()
            });

            match (obj.class().method_table.DefaultValue)(&mut obj, ctx, JsHint::Number) {
                Ok(val) => val.to_number(ctx),
                Err(e) => Err(e),
            }
        } else if unlikely(self.is_symbol()) {
            return Err(JsValue::new(
                ctx.new_type_error("Cannot convectx Symbol to number"),
            ));
        } else {
            unsafe { unreachable_unchecked() }
        }
    }

    pub fn is_callable(&self) -> bool {
        self.is_object()
            && self
                .get_object()
                .downcast::<JsObject>()
                .map(|object| object.is_callable())
                .unwrap_or(false)
    }

    pub fn is_primitive(&self) -> bool {
        self.is_number()
            || self.is_bool()
            || (self.is_object() && self.get_object().is::<JsString>())
            || (self.is_object() && self.get_object().is::<JsSymbol>())
    }

    pub fn to_string(&self, ctx: GcPointer<Context>) -> Result<String, JsValue> {
        if self.is_number() {
            Ok(self.get_number().to_string())
        } else if self.is_null() {
            Ok("null".to_string())
        } else if self.is_undefined() {
            Ok("undefined".to_string())
        } else if self.is_bool() {
            Ok(self.get_bool().to_string())
        } else if self.is_object() {
            let object = self.get_object();
            if let Some(jsstr) = object.downcast::<JsString>() {
                return Ok(jsstr.as_str().to_owned());
            } else if let Some(object) = object.downcast::<JsObject>() {
                let stack = ctx.shadowstack();
                letroot!(object = stack, object);
                return match object.to_primitive(ctx, JsHint::String) {
                    Ok(val) => val.to_string(ctx),
                    Err(e) => Err(e),
                };
            }
            if object.is::<SpreadValue>() {
                return Ok("spread".to_string());
            }
            if object.is::<JsSymbol>() {
                return Err(JsValue::new(
                    ctx.new_type_error("Cannot perform ToString on Symbol"),
                ));
            }
            println!("{:?}", (object.get_dyn()).type_name());
            todo!()
        } else {
            unreachable!("Should not be here")
        }
    }
    pub fn to_symbol(self, ctx: GcPointer<Context>) -> Result<Symbol, JsValue> {
        if self.is_object() && self.get_object().is::<JsSymbol>() {
            return Ok(self.get_object().downcast::<JsSymbol>().unwrap().symbol());
        }
        if self.is_number() {
            let n = self.get_number();
            if n as u32 as f64 == n {
                return Ok(Symbol::Index(n as u32));
            }
            return Ok(n.to_string().intern());
        }
        if self.is_jsstring() {
            return Ok(self.get_string().as_str().intern());
        }
        if self.is_null() {
            return Ok("null".intern());
        }
        if self.is_object() && self.get_object().is::<JsSymbol>() {
            return Ok(unsafe { self.get_object().downcast_unchecked::<JsSymbol>().symbol() });
        }

        if self.is_bool() {
            if self.get_bool() {
                return Ok("true".intern());
            } else {
                return Ok("false".intern());
            }
        }

        if self.is_undefined() {
            return Ok("undefined".intern());
        }
        let mut obj = self.get_object().downcast::<JsObject>().unwrap();
        let prim = obj.to_primitive(ctx, JsHint::String)?;
        prim.to_symbol(ctx)
    }

    pub fn get_primitive_proto(self, ctx: GcPointer<Context>) -> GcPointer<JsObject> {
        assert!(!self.is_empty());
        assert!(self.is_primitive());
        if self.is_jsstring() {
            return ctx.global_data().string_prototype.unwrap();
        } else if self.is_number() {
            return ctx.global_data().number_prototype.unwrap();
        } else if self.is_bool() {
            return ctx.global_data().boolean_prototype.unwrap();
        } else {
            return ctx.global_data().symbol_prototype.unwrap();
        }
    }

    pub fn get_jsobject(self) -> GcPointer<JsObject> {
        assert!(self.is_jsobject());
        unsafe { self.get_object().downcast_unchecked() }
    }
    pub fn get_jsstring(self) -> GcPointer<JsString> {
        assert!(self.is_jsstring());
        unsafe { self.get_object().downcast_unchecked() }
    }
    pub fn type_of(self) -> &'static str {
        if self.is_jsobject() {
            if self.is_callable() {
                return "function";
            }
            return "object";
        } else if self.is_number() {
            return "number";
        } else if self.is_jsstring() {
            return "string";
        } else if self.is_bool() {
            return "boolean";
        } else if self.is_undefined() {
            return "undefined";
        } else if self.is_null() {
            return "object";
        } else {
            return "symbol";
        }
    }
    pub fn get_slot(
        self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let stack = ctx.shadowstack();
        if !self.is_jsobject() {
            if self.is_null() {
                let msg = JsString::new(ctx, "null does not have properties");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }

            if self.is_undefined() {
                let d = ctx.description(name);
                let msg = JsString::new(
                    ctx,
                    &format!("undefined does not have properties ('{}')", d),
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }

            assert!(self.is_primitive());
            if self.is_jsstring() {
                let str = unsafe { self.get_object().downcast_unchecked::<JsString>() };

                if name == "length".intern() {
                    slot.set_1(
                        JsValue::new(str.len() as i32),
                        string_length(),
                        Some(str.as_dyn()),
                    );
                    return Ok(slot.value());
                }

                if let Symbol::Index(index) = name {
                    if index < str.len() {
                        let char = str
                            .as_str()
                            .chars()
                            .nth(index as usize)
                            .map(|x| {
                                JsValue::encode_object_value(JsString::new(ctx, x.to_string()))
                            })
                            .unwrap_or_else(JsValue::encode_undefined_value);
                        slot.set_1(char, string_indexed(), Some(str.as_dyn()));
                        return Ok(slot.value());
                    }
                }
            }
            letroot!(proto = stack, self.get_primitive_proto(ctx));
            if proto.get_property_slot(ctx, name, slot) {
                return slot.get(ctx, self);
            }
            return Ok(JsValue::encode_undefined_value());
        }
        letroot!(obj = stack, self.get_jsobject());
        obj.get_slot(ctx, name, slot)
    }

    pub fn to_boolean(self) -> bool {
        if self.is_number() {
            let num = self.get_number();
            return num != 0.0 && !num.is_nan();
        } else if self.is_jsstring() {
            return !self.get_jsstring().is_empty();
        } else if self.is_null() || self.is_undefined() {
            return false;
        } else if self.is_bool() {
            return self.get_bool();
        } else {
            true
        }
    }
    pub fn check_object_coercible(self, ctx: GcPointer<Context>) -> Result<(), Self> {
        if self.is_null() || self.is_undefined() {
            let msg = JsString::new(ctx, "null or undefined has no properties");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }
        Ok(())
    }
}
impl<T: JsClass> From<TypedJsObject<T>> for JsValue {
    fn from(x: TypedJsObject<T>) -> Self {
        Self::from(x.object())
    }
}
impl From<f64> for JsValue {
    fn from(x: f64) -> Self {
        Self::encode_untrusted_f64_value(x)
    }
}

use crate::gc::snapshot::deserializer::Deserializable;
impl GcCell for JsValue {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

pub fn print_value(x: JsValue) {
    if x.is_number() {
        print!("{}", x.get_number())
    } else if x.is_bool() {
        print!("{}", x.get_bool())
    } else if x.is_undefined() {
        print!("undefined");
    } else if x.is_null() {
        print!("null")
    } else if x.is_object() {
        print!("object");
    } else {
        print!("<>");
    }
}

macro_rules! from_primitive {
    ($($t: ty),*) => {$(
        impl From<$t> for JsValue {
            fn from(x: $t) -> Self {
                if x as i32 as $t == x {
                    return Self::encode_int32(x as _);
                }
                Self::encode_f64_value(x as f64)
            }
        })*
    };
}

from_primitive!(u8, i8, u16, i16, u32, i32, u64, i64);

impl From<f32> for JsValue {
    fn from(x: f32) -> Self {
        if x.is_nan() {
            return Self::encode_nan_value();
        }
        Self::encode_f64_value(x as _)
    }
}

impl<T: GcCell + ?Sized> From<GcPointer<T>> for JsValue {
    fn from(x: GcPointer<T>) -> Self {
        Self::encode_object_value(x)
    }
}

impl From<bool> for JsValue {
    fn from(x: bool) -> Self {
        Self::encode_bool_value(x)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Undefined;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Null;

impl From<Null> for JsValue {
    fn from(_x: Null) -> Self {
        Self::encode_null_value()
    }
}

impl From<Undefined> for JsValue {
    fn from(_x: Undefined) -> Self {
        Self::encode_undefined_value()
    }
}

impl TryFrom<JsValue> for i32 {
    type Error = &'static str;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        if value.is_number() {
            Ok(value.get_int32())
        } else if value.is_bool() {
            Ok(value.get_bool() as _)
        } else if value.is_null() {
            Ok(0)
        } else if value.is_jsstring() {
            let string = value.get_jsstring();
            string
                .as_str()
                .parse()
                .map_err(|_| "failed to parse JS string")
        } else {
            Err("Can not convectx JS value to i32")
        }
    }
}

impl TryFrom<JsValue> for f64 {
    type Error = &'static str;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        if value.is_number() {
            Ok(value.get_number())
        } else if value.is_null() {
            Ok(0.0)
        } else if value.is_bool() {
            Ok(value.get_bool() as i32 as f64)
        } else if value.is_undefined() {
            Ok(f64::NAN)
        } else if value.is_jsstring() {
            let string = value.get_jsstring();
            string
                .as_str()
                .parse()
                .map_err(|_| "failed to parse JS string")
        } else {
            Err("Can not convectx JS value to i32")
        }
    }
}
impl JsValue {
    pub fn new<T: Into<Self>>(x: T) -> Self {
        T::into(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new_i32() {
        let val = JsValue::new(42);
        assert!(val.is_number());
        assert_eq!(val.get_number(), 42f64);
    }

    #[test]
    fn test_new_f64() {
        let val = JsValue::new(f64::NAN);
        assert!(val.is_number());
        assert!(val.get_number().is_nan());
    }
}

pub struct HashValueZero(pub JsValue);

impl Hash for HashValueZero {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let value = self.0;
        if value.is_int32() {
            return value.get_int32().hash(state);
        }

        if value.is_number() {
            let d = value.get_number();
            if d.is_nan() {
                return std::f64::NAN.to_bits().hash(state);
            }
            if d == 0.0 {
                return 0i32.hash(state);
            }
            return d.to_bits().hash(state);
        }

        if value.is_jsstring() {
            let string = value.get_jsstring();
            return string.as_str().hash(state);
        }

        value.get_raw().hash(state);
    }
}

impl PartialEq for HashValueZero {
    fn eq(&self, other: &Self) -> bool {
        JsValue::same_value_zero(self.0, other.0)
    }
}

impl Eq for HashValueZero {}

unsafe impl Trace for HashValueZero {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.0.trace(visitor);
    }
}

impl GcCell for HashValueZero {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

impl Deserializable for HashValueZero {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        Self(JsValue::deserialize_inplace(deser))
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser));
    }
    unsafe fn allocate(ctx: &mut VirtualMachine, _deser: &mut Deserializer) -> *mut GcPointerBase {
        ctx.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}

impl Serializable for HashValueZero {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.0.serialize(serializer);
    }
}

pub mod new_value {
    //! TODO: This JS values results in nearly ~0.3% reduction in currently passing tests, we need to figure out why
    use std::fmt::Debug;

    use super::*;
    use wtf_rs::pure_nan::{pure_nan, purify_nan};
    #[derive(Copy, Clone, Debug)]
    pub struct JsValue(EncodedValueDescriptor);
    #[derive(Clone, Copy)]
    union EncodedValueDescriptor {
        as_int64: i64,
        #[cfg(target_pointer_width = "32")]
        as_double: f64,

        ptr: usize,
        #[cfg(target_pointer_width = "32")]
        as_bits: AsBits,
    }
    impl Debug for EncodedValueDescriptor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("EncodedValueDescriptor {}", unsafe {
                self.ptr
            }))
        }
    }

    impl PartialEq for JsValue {
        fn eq(&self, other: &Self) -> bool {
            unsafe { self.0.as_int64 == other.0.as_int64 }
        }
    }
    impl Eq for JsValue {}
    #[derive(Clone, Copy, PartialEq, Eq)]
    #[cfg(target_endian = "little")]
    #[repr(C)]
    struct AsBits {
        payload: i32,
        tag: i32,
    }
    #[derive(Clone, Copy, PactxialEq, Eq)]
    #[cfg(target_endian = "big")]
    #[repr(C)]
    struct AsBits {
        tag: i32,
        payload: i32,
    }

    #[cfg(target_pointer_width = "32")]
    impl JsValue {
        pub const INT32_TAG: u64 = 0xffffffff;
        pub const BOOLEAN_TAG: u64 = 0xfffffffe;
        pub const NULL_TAG: u64 = 0xfffffffd;
        pub const UNDEFINED_TAG: u64 = 0xfffffffc;
        pub const CELL_TAG: u64 = 0xfffffffb;
        pub const EMPTY_VALUE_TAG: u64 = 0xfffffffa;
        pub const DELETED_VALUE_TAG: u64 = 0xfffffff9;
        pub const LOWEST_TAG: u64 = Self::DELETED_VALUE_TAG;
        #[inline]
        pub fn encode_null_value() -> Self {
            Self(EncodedValueDescriptor {
                as_bits: AsBits {
                    tag: Self::NULL_TAG as _,
                    payload: 0,
                },
            })
        }
        #[inline]
        pub fn encode_undefined_value() -> Self {
            Self(EncodedValueDescriptor {
                as_bits: AsBits {
                    tag: Self::UNDEFINED_TAG as _,
                    payload: 0,
                },
            })
        }
        #[inline]
        pub fn encode_bool_value(x: bool) -> Self {
            Self(EncodedValueDescriptor {
                as_bits: AsBits {
                    tag: Self::BOOLEAN_TAG as _,
                    payload: x as _,
                },
            })
        }

        #[inline]
        pub fn encode_object_value<T: GcCell + ?Sized>(val: GcPointer<T>) -> Self {
            Self(EncodedValueDescriptor {
                as_bits: AsBits {
                    tag: Self::CELL_TAG as _,
                    payload: unsafe { std::mem::transmute(val) },
                },
            })
        }
        #[inline]
        pub fn encode_empty_value() -> Self {
            Self(EncodedValueDescriptor {
                as_bits: AsBits {
                    tag: Self::EMPTY_VALUE_TAG as _,
                    payload: 0,
                },
            })
        }
        #[inline]
        pub fn is_undefined(self) -> bool {
            self.tag() == Self::UNDEFINED_TAG as _
        }

        #[inline]
        pub fn is_null(self) -> bool {
            self.tag() == Self::NULL_TAG as _
        }
        #[inline]
        pub fn is_pointer(self) -> bool {
            self.tag() == Self::CELL_TAG as _
        }
        #[inline]
        pub fn is_int32(self) -> bool {
            self.tag() == Self::INT32_TAG as _
        }
        #[inline]
        pub fn is_double(self) -> bool {
            (self.tag() as u64) < Self::LOWEST_TAG
        }
        pub fn is_bool(self) -> bool {
            self.tag() == Self::BOOL_TAG as _
        }
        #[inline]
        pub fn get_bool(self) -> bool {
            assectx!(self.is_bool());
            self.payload() == 1
        }
        #[inline]
        pub fn is_empty(self) -> bool {
            self.tag() == Self::EMPTY_TAG as _
        }
        #[inline]
        pub fn tag(&self) -> i32 {
            unsafe { self.0.as_bits.tag }
        }
        #[inline]
        pub fn payload(&self) -> i32 {
            unsafe { self.0.as_bits.payload }
        }
    }
    #[cfg(target_pointer_width = "64")]
    impl JsValue {
        /*
         * On 64-bit platforms USE(JSVALUE64) should be defined, and we use a NaN-encoded
         * form for immediates.
         *
         * The encoding makes use of unused NaN space in the IEEE754 representation.  Any value
         * with the top 13 bits set represents a QNaN (with the sign bit set).  QNaN values
         * can encode a 51-bit payload.  Hardware produced and C-library payloads typically
         * have a payload of zero.  We assume that non-zero payloads are available to encode
         * pointer and integer values.  Since any 64-bit bit pattern where the top 15 bits are
         * all set represents a NaN with a non-zero payload, we can use this space in the NaN
         * ranges to encode other values (however there are also other ranges of NaN space that
         * could have been selected).
         *
         * This range of NaN space is represented by 64-bit numbers begining with the 15-bit
         * hex patterns 0xFFFC and 0xFFFE - we rely on the fact that no valid double-precision
         * numbers will fall in these ranges.
         *
         * The top 15-bits denote the type of the encoded JSValue:
         *
         *     Pointer {  0000:PPPP:PPPP:PPPP
         *              / 0002:****:****:****
         *     Double  {         ...
         *              \ FFFC:****:****:****
         *     Integer {  FFFE:0000:IIII:IIII
         *
         * The scheme we have implemented encodes double precision values by performing a
         * 64-bit integer addition of the value 2^49 to the number. After this manipulation
         * no encoded double-precision value will begin with the pattern 0x0000 or 0xFFFE.
         * Values must be decoded by reversing this operation before subsequent floating point
         * operations may be peformed.
         *
         * 32-bit signed integers are marked with the 16-bit tag 0xFFFE.
         *
         * The tag 0x0000 denotes a pointer, or another form of tagged immediate. Boolean,
         * null and undefined values are represented by specific, invalid pointer values:
         *
         *     False:     0x06
         *     True:      0x07
         *     Undefined: 0x0a
         *     Null:      0x02
         *
         * These values have the following properties:
         * - Bit 1 (Othectxag) is set for all four values, allowing real pointers to be
         *   quickly distinguished from all immediate values, including these invalid pointers.
         * - With bit 3 masked out (UndefinedTag), Undefined and Null share the
         *   same value, allowing null & undefined to be quickly detected.
         *
         * No valid JSValue will have the bit pattern 0x0, this is used to represent array
         * holes, and as a C++ 'no value' result (e.g. JSValue() has an internal value of 0).
         *
         * When USE(BIGINT32), we have a special representation for BigInts that are small (32-bit at most):
         *      0000:XXXX:XXXX:0012
         * This representation works because of the following things:
         * - It cannot be confused with a Double or Integer thanks to the top bits
         * - It cannot be confused with a pointer to a Cell, thanks to bit 1 which is set to true
         * - It cannot be confused with a pointer to wasm thanks to bit 0 which is set to false
         * - It cannot be confused with true/false because bit 2 is set to false
         * - It cannot be confused for null/undefined because bit 4 is set to true
         */

        pub const DOUBLE_ENCODE_OFFSET_BIT: usize = 49;
        pub const DOUBLE_ENCODE_OFFSET: i64 = 1 << Self::DOUBLE_ENCODE_OFFSET_BIT as i64;
        pub const NUMBER_TAG: i64 = 0xfffe000000000000u64 as i64;
        pub const LOWEST_OF_HIGH_BITS: i64 = 1 << 49;

        pub const OTHER_TAG: i32 = 0x2;
        pub const BOOL_TAG: i32 = 0x4;
        pub const UNDEFINED_TAG: i32 = 0x8;
        pub const NATIVE32_TAG: i32 = 0x12;
        pub const NATIVE32_MASK: i64 = Self::NUMBER_TAG | Self::NATIVE32_TAG as i64;

        pub const VALUE_FALSE: i32 = Self::OTHER_TAG | Self::BOOL_TAG | false as i32;
        pub const VALUE_TRUE: i32 = Self::OTHER_TAG | Self::BOOL_TAG | true as i32;
        pub const VALUE_UNDEFINED: i32 = Self::OTHER_TAG | Self::UNDEFINED_TAG;
        pub const VALUE_NULL: i32 = Self::OTHER_TAG;

        pub const MISC_TAG: i64 =
            Self::OTHER_TAG as i64 | Self::BOOL_TAG as i64 | Self::UNDEFINED_TAG as i64;
        pub const NOT_CELL_MASK: i64 = Self::NUMBER_TAG as i64 | Self::OTHER_TAG as i64;

        pub const VALUE_EMPTY: i64 = 0x0;
        pub const VALUE_DELETED: i64 = 0x4;

        pub const UNDEFINED: new_value::JsValue = Self::encode_undefined_value();

        #[inline]
        pub fn encode_empty_value() -> Self {
            Self(EncodedValueDescriptor {
                as_int64: Self::VALUE_EMPTY,
            })
        }
        #[inline]
        pub fn encode_object_value<T: GcCell + ?Sized>(gc: GcPointer<T>) -> Self {
            Self(EncodedValueDescriptor {
                ptr: gc.base.as_ptr() as usize,
            })
        }

        #[inline]
        pub const fn encode_undefined_value() -> Self {
            Self(EncodedValueDescriptor {
                as_int64: Self::VALUE_UNDEFINED as _,
            })
        }

        #[inline]
        pub const fn encode_null_value() -> Self {
            Self(EncodedValueDescriptor {
                as_int64: Self::VALUE_NULL as _,
            })
        }

        #[inline]
        pub fn encode_bool_value(x: bool) -> Self {
            if x {
                Self(EncodedValueDescriptor {
                    as_int64: Self::VALUE_TRUE as _,
                })
            } else {
                Self(EncodedValueDescriptor {
                    as_int64: Self::VALUE_FALSE as _,
                })
            }
        }
        #[inline]
        pub fn is_empty(self) -> bool {
            unsafe { self.0.as_int64 == Self::VALUE_EMPTY }
        }

        #[inline]
        pub fn is_undefined(self) -> bool {
            self == Self::encode_undefined_value()
        }
        #[inline]
        pub fn is_null(self) -> bool {
            self == Self::encode_null_value()
        }

        #[inline]
        pub fn is_true(self) -> bool {
            self == Self::encode_bool_value(true)
        }

        #[inline]
        pub fn is_false(self) -> bool {
            self == Self::encode_bool_value(false)
        }

        #[inline]
        pub fn is_boolean(self) -> bool {
            unsafe { (self.0.as_int64 & !1) == Self::VALUE_FALSE as i64 }
        }

        #[inline]
        pub fn is_pointer(self) -> bool {
            unsafe { (self.0.as_int64 & Self::NOT_CELL_MASK) == 0 }
        }

        #[inline]
        pub fn is_int32(self) -> bool {
            unsafe { (self.0.as_int64 & Self::NUMBER_TAG) == Self::NUMBER_TAG }
        }

        #[inline]
        pub fn is_number(self) -> bool {
            unsafe { (self.0.as_int64 & Self::NUMBER_TAG) != 0 }
        }

        #[inline]
        pub fn get_object(self) -> GcPointer<dyn GcCell> {
            assert!(self.is_object());

            unsafe { std::mem::transmute(self.0.ptr) }
        }

        #[inline]
        pub fn is_object(self) -> bool {
            self.is_pointer() && !self.is_empty()
        }
        #[inline]
        pub fn get_int32(self) -> i32 {
            unsafe { self.0.as_int64 as i32 }
        }

        #[inline]
        pub fn get_number(self) -> f64 {
            if self.is_int32() {
                return self.get_int32() as _;
            }
            self.get_double()
        }
        #[inline]
        pub fn get_double(self) -> f64 {
            assert!(self.is_double());
            f64::from_bits((unsafe { self.0.as_int64 - Self::DOUBLE_ENCODE_OFFSET }) as u64)
        }
        #[inline]
        pub fn is_double(self) -> bool {
            self.is_number() && !self.is_int32()
        }

        #[inline]
        pub fn is_bool(self) -> bool {
            unsafe { (self.0.as_int64 & !1) == Self::VALUE_FALSE as i64 }
        }

        #[inline]
        pub fn encode_f64_value(x: f64) -> Self {
            Self(EncodedValueDescriptor {
                as_int64: x.to_bits() as i64 + Self::DOUBLE_ENCODE_OFFSET,
            })
        }

        #[inline]
        pub fn encode_untrusted_f64_value(x: f64) -> Self {
            Self::encode_f64_value(purify_nan(x))
        }

        #[inline]
        pub fn encode_nan_value() -> Self {
            Self::encode_f64_value(pure_nan())
        }

        #[inline]
        pub fn encode_int32(x: i32) -> Self {
            Self(EncodedValueDescriptor {
                as_int64: Self::NUMBER_TAG | x as u32 as u64 as i64,
            })
        }

        #[inline]
        pub fn get_raw(self) -> i64 {
            unsafe { self.0.as_int64 }
        }

        #[inline]
        pub fn get_native_u32(self) -> u32 {
            unsafe { (self.0.as_int64 >> 16) as u32 }
        }

        #[inline]
        pub fn encode_native_u32(x: u32) -> Self {
            Self(EncodedValueDescriptor {
                as_int64: (((x as u64) << 16) | Self::NATIVE32_TAG as u64) as i64,
            })
        }
        #[inline]
        pub fn is_native_value(self) -> bool {
            unsafe { (self.0.as_int64 & Self::NATIVE32_MASK) == Self::NATIVE32_TAG as i64 }
        }

        #[inline]
        pub fn get_bool(self) -> bool {
            assert!(self.is_bool());
            self == Self::encode_bool_value(true)
        }
    }
}

pub trait JsFrom<T> {
    fn js_from(ctx: GcPointer<Context>, val: T) -> JsValue;
}

impl<T: Into<JsValue>> JsFrom<T> for JsValue {
    fn js_from(_ctx: GcPointer<Context>, val: T) -> JsValue {
        JsValue::new(val)
    }
}

impl JsFrom<&str> for JsValue {
    fn js_from(ctx: GcPointer<Context>, val: &str) -> JsValue {
        JsValue::new(JsString::new(ctx, val))
    }
}
impl JsFrom<String> for JsValue {
    fn js_from(ctx: GcPointer<Context>, val: String) -> JsValue {
        JsValue::new(JsString::new(ctx, val))
    }
}
