use crate::gc::cell::*;
use cfg_if::cfg_if;
use std::{
    convert::TryFrom,
    hint::unreachable_unchecked,
    intrinsics::{likely, unlikely},
};

use super::{
    attributes::*,
    error::*,
    number::*,
    object::{JsHint, JsObject},
    slot::*,
    string::*,
    symbol_table::*,
    Runtime,
};

pub type TagKind = u32;
pub const CMP_FALSE: i32 = 0;
pub const CMP_TRUE: i32 = 1;
pub const CMP_UNDEF: i32 = -1;
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

cfg_if!(
    if #[cfg(feature="val-as-u64")] {
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
        Self::internal_new(unsafe {std::mem::transmute::<_,usize>(val)} as _, OBJECT_TAG)
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
    pub fn is_string(&self) -> bool {
        self.get_tag() == STR_TAG
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
        unsafe { std::mem::transmute(self.0 & Self::DATA_MASK) }
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
        unsafe { std::mem::transmute::<_,GcPointer<dyn GcCell>>(self.0 & Self::DATA_MASK) }.clone()
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

    } else if #[cfg(feature="val-as-f64")] {
/// A NaN-boxed encoded value.

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JsValue(f64);
impl PartialEq for JsValue {
    fn eq(&self,other: &Self) -> bool {
        self.get_raw() == other.get_raw()
    }
}
impl Eq for JsValue {}
impl JsValue {
    pub const NUM_TAG_EXP_BITS: u32 = 16;
    pub const NUM_DATA_BITS: u32 = (64 - Self::NUM_TAG_EXP_BITS);
    pub const TAG_WIDTH: u32 = 4;
    pub const TAG_MASK: u32 = (1 << Self::TAG_WIDTH) - 1;
    pub const DATA_MASK: u64 = (1 << Self::NUM_DATA_BITS as u64) - 1;
    pub const ETAG_WIDTH: u32 = 5;
    pub const ETAG_MASK: u32 = (1 << Self::ETAG_WIDTH) - 1;
    #[inline]
    pub fn from_raw(x: u64) -> Self {
        Self(f64::from_bits(x))
    }
    #[inline]
    pub fn get_tag(&self) -> TagKind {
        (self.0.to_bits() >> Self::NUM_DATA_BITS as u64) as u32
    }
    #[inline]
    pub fn get_etag(&self) -> ExtendedTag {
        unsafe { std::mem::transmute((self.0.to_bits() >> (Self::NUM_DATA_BITS as u64 - 1)) as u32) }
    }
    #[inline]
    pub fn combine_tags(a: TagKind, b: TagKind) -> u32 {
        ((a & Self::TAG_MASK) << Self::TAG_WIDTH) | (b & Self::TAG_MASK)
    }
    #[inline]
    fn internal_new(val: u64, tag: TagKind) -> Self {
        Self(f64::from_bits(val | ((tag as u64) << Self::NUM_DATA_BITS)))
    }
    #[inline]
    fn new_extended(val: u64, tag: ExtendedTag) -> Self {
        Self(f64::from_bits(val | ((tag as u64) << (Self::NUM_DATA_BITS - 1))))
    }
    #[inline]
    pub fn encode_null_ptr_object_value() -> Self {
        Self::internal_new(0, OBJECT_TAG)
    }
    #[inline]
    pub fn encode_object_value<T: GcCell + ?Sized>(val: GcPointer<T>) -> Self {
        Self::internal_new(unsafe {std::mem::transmute::<_,usize>(val)} as _, OBJECT_TAG)
    }
    #[inline]
    pub fn encode_native_u32(val: u32) -> Self {
        Self::internal_new(val as _, NATIVE_VALUE_TAG)
    }
    #[inline]
    pub fn encode_native_pointer(p: *const ()) -> Self {
        Self::internal_new(p as _, NATIVE_VALUE_TAG)
    }
    #[inline]
    pub fn encode_bool_value(val: bool) -> Self {
        Self::internal_new(val as _, BOOL_TAG)
    }
    #[inline]
    pub fn encode_null_value() -> Self {
        Self::new_extended(0, ExtendedTag::Null)
    }
    #[inline]
    pub fn encode_int32(x: i32) -> Self {
        Self::internal_new(x as u32 as u64, INT32_TAG)
    }
    #[inline]
    pub fn encode_undefined_value() -> Self {
        Self::new_extended(0, ExtendedTag::Undefined)
    }
    #[inline]
    pub fn encode_empty_value() -> Self {
        Self::new_extended(0, ExtendedTag::Empty)
    }
    #[inline]
    pub fn encode_f64_value(x: f64) -> Self {
        Self::from_raw(x.to_bits())
    }

    #[inline]
    pub fn encode_nan_value() -> Self {
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
        self.0 = f64::from_bits(val as u64 | (self.get_tag() as u64) << Self::NUM_DATA_BITS as u64);
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
        self.is_native_value()
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
    pub fn is_string(&self) -> bool {
        self.get_tag() == STR_TAG
    }

    #[inline]
    pub fn is_double(&self) -> bool {
        self.0.to_bits() < ((FIRST_TAG as u64) << Self::NUM_DATA_BITS as u64)
    }

    #[inline]
    pub fn is_pointer(&self) -> bool {
        self.0.to_bits() >= ((FIRST_PTR_TAG as u64) << Self::NUM_DATA_BITS as u64)
    }

    #[inline]
    pub fn get_raw(&self) -> u64 {
        self.0.to_bits()
    }

    #[inline]
    pub fn get_pointer(&self) -> *mut () {
        assert!(self.is_pointer());
        unsafe { std::mem::transmute(self.0.to_bits() & Self::DATA_MASK) }
    }
    #[inline]
    pub fn get_int32(&self) -> i32 {
        assert!(self.is_int32());
        self.0 as u32 as i32
    }
    #[inline]
    pub fn get_double(&self) -> f64 {
        if self.is_int32() {
            return self.get_int32() as i32;
        }
        f64::from_bits(self.0.to_bits())
    }
    #[inline]
    pub fn get_native_value(&self) -> i64 {
        assert!(self.is_native_value());
        (((self.0.to_bits() & Self::DATA_MASK as u64) as i64) << (64 - Self::NUM_DATA_BITS as i64))
            >> (64 - Self::NUM_DATA_BITS as i64)
    }

    #[inline]
    pub fn get_native_u32(&self) -> u32 {
        assert!(self.is_native_value());
        self.0.to_bits() as u32
    }

    #[inline]
    pub fn get_native_ptr(&self) -> *mut () {
        assert!(self.is_native_value());
        (self.0.to_bits() & Self::DATA_MASK) as *mut ()
    }

    #[inline]
    pub fn get_bool(&self) -> bool {
        assert!(self.is_bool());
        (self.0.to_bits() & 0x1) != 0
    }

    #[inline]
    pub fn get_object(&self) -> GcPointer<dyn GcCell> {
        assert!(self.is_object());
        unsafe { std::mem::transmute::<_,GcPointer<dyn GcCell>>(self.0.to_bits() & Self::DATA_MASK) }.clone()
    }

    #[inline]
    pub fn get_number(&self) -> f64 {
        self.get_double()
    }

    pub unsafe fn set_no_barrier(&mut self, val: Self) {
        self.0 = val.0;
    }

    pub fn is_number(&self) -> bool {
        self.is_double() || self.is_int32()
    }
}

    } else {
        compile_error!("val-as-u64 or val-as-f64 should be enabled");
    }
);

unsafe impl Trace for JsValue {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        if self.is_object() && !self.is_empty() && !self.is_int32() {
            *self = JsValue::encode_object_value(visitor.visit(&mut self.get_object()));
        }
    }
}

impl JsValue {
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
        if lhs.is_object() && rhs.is_object() {
            if lhs.get_object().is::<JsString>() && rhs.get_object().is::<JsString>() {
                return unsafe {
                    lhs.get_object().downcast_unchecked::<JsString>().as_str()
                        == rhs.get_object().downcast_unchecked::<JsString>().as_str()
                };
            }
        }
        lhs.get_raw() == rhs.get_raw()
    }
    pub fn same_value(x: JsValue, y: JsValue) -> bool {
        Self::same_value_impl(x, y, false)
    }
    pub fn same_value_zero(lhs: Self, rhs: Self) -> bool {
        Self::same_value_impl(lhs, rhs, true)
    }
    pub fn to_object(self, rt: &mut Runtime) -> Result<GcPointer<JsObject>, JsValue> {
        if self.is_undefined() || self.is_null() {
            let msg = JsString::new(rt, "ToObject to null or undefined");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }
        if self.is_object() && self.get_object().is::<JsObject>() {
            return Ok(unsafe { self.get_object().downcast_unchecked() });
        }
        if self.is_number() {
            return Ok(NumberObject::new(rt, self.get_number()));
        }
        if self.is_jsstring() {
            return Ok(JsStringObject::new(rt, self.get_jsstring()));
        }
        todo!()
    }
    pub fn is_jsobject(self) -> bool {
        self.is_object() && self.get_object().is::<JsObject>()
    }
    pub fn is_symbol(self) -> bool {
        self.is_object() && self.get_object().is::<JsSymbol>()
    }
    pub fn to_primitive(self, rt: &mut Runtime, hint: JsHint) -> Result<JsValue, JsValue> {
        if self.is_object() && self.get_object().is::<JsObject>() {
            let mut object = unsafe { self.get_object().downcast_unchecked::<JsObject>() };
            object.to_primitive(rt, hint)
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
        assert!(self.is_string() || self.is_jsstring());
        unsafe { self.get_object().downcast_unchecked() }
    }
    pub fn is_jsstring(self) -> bool {
        self.is_object() && self.get_object().is::<JsString>()
    }

    pub fn abstract_equal(self, other: JsValue, rt: &mut Runtime) -> Result<bool, JsValue> {
        let mut lhs = self;
        let mut rhs = other;

        loop {
            if likely(lhs.is_number() && rhs.is_number()) {
                return Ok(self.get_number() == rhs.get_number());
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
                rhs = JsValue::new(rhs.to_number(rt)?);
                continue;
            }

            if lhs.is_bool() {
                lhs = JsValue::new(lhs.to_number(rt)?);
                continue;
            }

            if rhs.is_bool() {
                rhs = JsValue::new(rhs.to_number(rt)?);
                continue;
            }

            if (lhs.is_jsstring() || lhs.is_number()) && rhs.is_object() {
                rhs = rhs.to_primitive(rt, JsHint::None)?;
                continue;
            }

            if lhs.is_object() && (rhs.is_jsstring() || rhs.is_number()) {
                lhs = lhs.to_primitive(rt, JsHint::None)?;
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
    pub fn compare(self, rhs: Self, left_first: bool, rt: &mut Runtime) -> Result<i32, JsValue> {
        let lhs = self;
        if likely(lhs.is_number() && rhs.is_number()) {
            return Ok(Self::number_compare(self.get_number(), rhs.get_number()));
        }

        let px;
        let py;
        if left_first {
            px = lhs.to_primitive(rt, JsHint::Number)?;
            py = rhs.to_primitive(rt, JsHint::Number)?;
        } else {
            py = rhs.to_primitive(rt, JsHint::Number)?;
            px = lhs.to_primitive(rt, JsHint::Number)?;
        }
        if likely(px.is_number() && py.is_number()) {
            return Ok(Self::number_compare(px.get_number(), py.get_number()));
        }
        if likely(px.is_jsstring() && py.is_jsstring()) {
            if px.get_string().as_str().len() < py.get_string().as_str().len() {
                Ok(CMP_TRUE)
            } else {
                Ok(CMP_FALSE)
            }
        } else {
            let nx = px.to_number(rt)?;
            let ny = py.to_number(rt)?;
            Ok(Self::number_compare(nx, ny))
        }
    }
    pub fn compare_left(self, rhs: Self, rt: &mut Runtime) -> Result<i32, JsValue> {
        Self::compare(self, rhs, true, rt)
    }
    pub fn to_int32(self, rt: &mut Runtime) -> Result<i32, JsValue> {
        if self.is_int32() {
            return Ok(self.get_int32());
        }
        let number = self.to_number(rt)?;
        if unlikely(number.is_nan() || number.is_infinite()) {
            return Ok(0);
        }
        Ok(number.floor() as i32)
    }

    pub fn to_uint32(self, rt: &mut Runtime) -> Result<u32, JsValue> {
        if self.is_int32() {
            return Ok(self.get_int32() as _);
        }
        let number = self.to_number(rt)?;
        if unlikely(number.is_nan() || number.is_infinite()) {
            return Ok(0);
        }
        Ok(number.floor() as u32)
    }

    pub fn to_number(self, rt: &mut Runtime) -> Result<f64, JsValue> {
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
            let stack = rt.shadowstack();
            letroot!(obj = stack, unsafe {
                self.get_object().downcast_unchecked::<JsObject>()
            });

            match (obj.class().method_table.DefaultValue)(&mut obj, rt, JsHint::Number) {
                Ok(val) => val.to_number(rt),
                Err(e) => Err(e),
            }
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

    pub fn to_string(&self, rt: &mut Runtime) -> Result<String, JsValue> {
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
            if let Some(jsstr) = object.clone().downcast::<JsString>() {
                return Ok(jsstr.as_str().to_owned());
            } else if let Some(object) = object.downcast::<JsObject>() {
                let stack = rt.shadowstack();
                letroot!(object = stack, object);
                return match object.to_primitive(rt, JsHint::String) {
                    Ok(val) => val.to_string(rt),
                    Err(e) => Err(e),
                };
            }

            todo!()
        } else {
            unreachable!()
        }
    }
    pub fn to_symbol(self, rt: &mut Runtime) -> Result<Symbol, JsValue> {
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
        let prim = obj.to_primitive(rt, JsHint::String)?;
        prim.to_symbol(rt)
    }

    pub fn get_primitive_proto(self, vm: &mut Runtime) -> GcPointer<JsObject> {
        assert!(!self.is_empty());
        assert!(self.is_primitive());
        if self.is_jsstring() {
            return vm.global_data().string_prototype.clone().unwrap();
        } else if self.is_number() {
            return vm.global_data().number_prototype.clone().unwrap();
        } else if self.is_bool() {
            return vm.global_data().boolean_prototype.clone().unwrap();
        } else {
            return vm.global_data().symbol_prototype.clone().unwrap();
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
        rt: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let stack = rt.shadowstack();
        if !self.is_jsobject() {
            if self.is_null() {
                let msg = JsString::new(rt, "null does not have properties");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    rt, msg, None,
                )));
            }

            if self.is_undefined() {
                let d = rt.description(name);
                let msg =
                    JsString::new(rt, &format!("undefined does not have properties ('{}')", d));
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    rt, msg, None,
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
                            .map(|x| JsValue::encode_object_value(JsString::new(rt, x.to_string())))
                            .unwrap_or(JsValue::encode_undefined_value());
                        slot.set_1(char, string_indexed(), Some(str.as_dyn()));
                        return Ok(slot.value());
                    }
                }
            }
            letroot!(proto = stack, self.get_primitive_proto(rt));
            if proto.get_property_slot(rt, name, slot) {
                return slot.get(rt, self);
            }
            return Ok(JsValue::encode_undefined_value());
        }
        letroot!(obj = stack, self.get_jsobject());
        obj.get_slot(rt, name, slot)
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
            return true;
        }
    }
    pub fn check_object_coercible(self, rt: &mut Runtime) -> Result<(), Self> {
        if self.is_null() || self.is_undefined() {
            let msg = JsString::new(rt, "null or undefined has no properties");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }
        Ok(())
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
    vtable_impl!();
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

impl From<()> for JsValue {
    fn from(_x: ()) -> Self {
        Self::encode_undefined_value()
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
            Err("Can not convert JS value to i32")
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
            Err("Can not convert JS value to i32")
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
