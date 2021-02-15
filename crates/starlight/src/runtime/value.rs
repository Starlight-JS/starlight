use super::{attributes::string_length, slot::*};
use crate::{
    heap::cell::{Cell, Gc, Trace, Tracer},
    vm::VirtualMachine,
};

use std::mem::transmute;
use wtf_rs::{object_offsetof, pure_nan};
pub const CMP_FALSE: i32 = 0;
pub const CMP_TRUE: i32 = 1;
pub const CMP_UNDEF: i32 = -1;
use super::{
    error::JsTypeError,
    object::{JsHint, JsObject},
    string::JsString,
    symbol::{JsSymbol, Symbol},
};

#[derive(Clone, Copy)]
#[repr(C)]
union EncodedValueDescriptor {
    as_int64: i64,
    as_uint64: u64,
    #[cfg(target_pointer_width = "32")]
    as_double: f64,
    #[cfg(target_pointer_width = "64")]
    ptr: Gc<dyn Cell>,
    as_bits: AsBits,
}

#[cfg(target_endian = "big")]
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct AsBits {
    tag: i32,
    payload: i32,
}
#[cfg(target_endian = "little")]
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct AsBits {
    payload: i32,
    tag: i32,
}

pub fn tag_offset() -> usize {
    object_offsetof!(AsBits, tag)
}

pub fn payload_offset() -> usize {
    object_offsetof!(AsBits, payload)
}

pub enum WhichValueWord {
    Tag,
    Payload,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JsValue {
    u: EncodedValueDescriptor,
}

#[cfg(target_pointer_width = "64")]
impl JsValue {
    /*
     * On 64-bit platforms we use a NaN-encoded
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
     * The top 15-bits denote the type of the encoded JsValue:
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
     * - Bit 1 (OtherTag) is set for all four values, allowing real pointers to be
     *   quickly distinguished from all immediate values, including these invalid pointers.
     * - With bit 3 masked out (UndefinedTag), Undefined and Null share the
     *   same value, allowing null & undefined to be quickly detected.
     *
     * No valid JsValue will have the bit pattern 0x0, this is used to represent array
     * holes, and as a Rust 'no value' result (e.g. JsValue::empty() has an internal value of 0).
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

    /// This value is 2^49, used to encode doubles such that the encoded value will begin
    /// with a 15-bit pattern within the range 0x0002..0xFFFC.
    pub const DOUBLE_ENCODE_OFFSET_BIT: usize = 49;
    pub const DOUBLE_ENCODE_OFFSET: u64 = 1 << Self::DOUBLE_ENCODE_OFFSET_BIT as u64;
    pub const NUMBER_TAG: u64 = 0xfffe000000000000;
    pub const LOWEST_OF_HIGH_BITS: u64 = 1 << 49;
    pub const OTHER_TAG: u64 = 0x2;
    pub const BOOL_TAG: u64 = 0x4;
    pub const UNDEFINED_TAG: u64 = 0x8;
    pub const VALUE_FALSE: u64 = Self::OTHER_TAG | Self::BOOL_TAG | 0;
    pub const VALUE_TRUE: u64 = Self::OTHER_TAG | Self::BOOL_TAG | 1;
    pub const VALUE_UNDEFINED: u64 = Self::OTHER_TAG | Self::UNDEFINED_TAG;
    pub const VALUE_NULL: u64 = Self::OTHER_TAG;
    pub const MISC_TAG: u64 = Self::OTHER_TAG | Self::BOOL_TAG | Self::UNDEFINED_TAG;
    // NOT_CELL_MASK is used to check for all types of immediate values (either number or 'other').
    pub const NOT_CELL_MASK: u64 = Self::NUMBER_TAG | Self::OTHER_TAG;

    /// These special values are never visible to JavaScript code; Empty is used to represent
    /// Array holes, and for uninitialized JsValues. Deleted is used in hash table code.
    /// These values would map to cell types in the JsValue encoding, but not valid GC cell
    /// pointer should have either of these values (Empty is null, deleted is at an invalid
    /// alignment for a GC cell, and in the zero page).
    pub const VALUE_EMPTY: u64 = 0x0;
    pub const VALUE_DELETED: u64 = 0x4;

    pub fn empty() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_EMPTY,
            },
        }
    }

    pub fn deleted() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_DELETED,
            },
        }
    }

    pub fn undefined() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_UNDEFINED,
            },
        }
    }

    pub fn null() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_NULL,
            },
        }
    }

    pub fn true_() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_TRUE,
            },
        }
    }
    pub fn false_() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_uint64: Self::VALUE_FALSE,
            },
        }
    }

    pub fn new<T: Into<Self>>(x: T) -> Self {
        x.into()
    }

    pub fn is_empty(self) -> bool {
        self == Self::empty()
    }

    pub fn is_undefined(self) -> bool {
        self == Self::undefined()
    }

    pub fn is_null(self) -> bool {
        self == Self::null()
    }

    pub fn is_true(self) -> bool {
        self == Self::true_()
    }

    pub fn is_false(self) -> bool {
        self == Self::false_()
    }

    pub fn as_boolean(self) -> bool {
        assert!(self.is_boolean());
        self == Self::true_()
    }
    #[inline(always)]
    pub fn as_int32(self) -> i32 {
        assert!(self.is_int32());
        unsafe { self.u.as_int64 as i32 }
    }

    pub fn is_double(self) -> bool {
        self.is_number() && !self.is_int32()
    }

    pub fn is_undefined_or_null(self) -> bool {
        unsafe { (self.u.as_int64 & Self::UNDEFINED_TAG as i64) == Self::VALUE_NULL as i64 }
    }

    pub fn is_boolean(self) -> bool {
        unsafe { (self.u.as_int64 & !1) == Self::VALUE_FALSE as i64 }
    }

    pub fn is_cell(self) -> bool {
        unsafe { (self.u.as_int64 & Self::NOT_CELL_MASK as i64) == 0 }
    }

    pub fn is_int32(self) -> bool {
        unsafe { (self.u.as_int64 & Self::NUMBER_TAG as i64) == Self::NUMBER_TAG as i64 }
    }
    #[inline(always)]
    pub fn as_double(self) -> f64 {
        //  assert!(self.is_double());
        unsafe { transmute(self.u.as_int64 - Self::DOUBLE_ENCODE_OFFSET as i64) }
    }

    pub fn is_number(self) -> bool {
        unsafe { (self.u.as_int64 & Self::NUMBER_TAG as i64) != 0 }
    }

    pub fn as_cell(self) -> Gc<dyn Cell> {
        // TODO(playX): we might want to insert is_empty check here too?
        assert!(self.is_cell());
        unsafe { self.u.ptr }
    }
}

impl JsValue {
    pub fn number(self) -> f64 {
        if self.is_int32() {
            self.as_int32() as f64
        } else if self.is_double() {
            self.as_double()
        } else {
            pure_nan::pure_nan()
        }
    }
    pub fn to_number(self, _vm: &mut VirtualMachine) -> Result<f64, JsValue> {
        if self.is_number() {
            Ok(self.number())
        } else if self.is_cell() && self.as_cell().is::<JsString>() {
            unsafe {
                let s = self.as_cell().downcast_unchecked::<JsString>();
                Ok(s.as_str()
                    .parse::<f64>()
                    .unwrap_or_else(|_| pure_nan::pure_nan()))
            }
        } else if self.is_boolean() {
            if self.as_boolean() {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        } else if self.is_null() {
            Ok(0.0)
        } else if self.is_undefined() {
            Ok(pure_nan::pure_nan())
        } else if self.is_cell() && self.as_cell().is::<JsObject>() {
            let obj = unsafe { self.as_cell().downcast_unchecked::<JsObject>() };
            match (obj.get_class_value().unwrap().method_table.DefaultValue)(
                obj,
                _vm,
                JsHint::Number,
            ) {
                Ok(val) => val.to_number(_vm),
                Err(e) => Err(e),
            }
        } else {
            assert!(!self.is_empty());
            todo!()
        }
    }
    pub fn is_callable(self) -> bool {
        !self.is_empty()
            && self.is_cell()
            && self
                .as_cell()
                .downcast::<JsObject>()
                .map(|x| x.is_callable())
                .unwrap_or(false)
    }
    pub fn is_primitive(self) -> bool {
        self.is_number()
            || self.is_boolean()
            || (self.is_cell() && self.as_cell().is::<JsSymbol>())
            || (self.is_cell() && self.as_cell().is::<JsString>())
    }

    pub fn to_string(self, vm: &mut VirtualMachine) -> Result<String, JsValue> {
        if self.is_number() {
            Ok(if self.is_int32() {
                self.as_int32().to_string()
            } else {
                self.as_double().to_string()
            })
        } else if self.is_null() {
            Ok("null".to_owned())
        } else if self.is_undefined() {
            Ok("undefined".to_string())
        } else if self.is_boolean() {
            Ok(self.as_boolean().to_string())
        } else if self.is_cell() && !self.is_empty() {
            let cell = self.as_cell();
            if let Some(jsstr) = cell.downcast::<JsString>() {
                return Ok(jsstr.as_str().to_owned());
            } else if let Some(mut obj) = cell.downcast::<JsObject>() {
                return match obj.to_primitive(vm, JsHint::String) {
                    Ok(val) => val.to_string(vm),
                    Err(e) => Err(e),
                };
            } else if cell.downcast::<JsSymbol>().is_some() {
                let msg = JsString::new(vm, "cannot convert Symbol to string");
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            }
            todo!()
        } else {
            assert!(!self.is_empty());
            unreachable!()
        }
    }
    pub fn to_primitive(self, vm: &mut VirtualMachine, hint: JsHint) -> Result<Self, Self> {
        if self.is_cell() && self.as_cell().is::<JsObject>() {
            let mut obj = self.as_cell().downcast::<JsObject>().unwrap();
            obj.to_primitive(vm, hint)
        } else {
            Ok(self)
        }
    }

    pub fn is_string(self) -> bool {
        self.is_cell() && self.as_cell().is::<JsString>()
    }
    pub fn as_string(self) -> Gc<JsString> {
        assert!(self.is_string());
        unsafe { self.as_cell().downcast_unchecked() }
    }
    pub fn is_object(self) -> bool {
        self.is_cell() && self.as_cell().is::<JsObject>()
    }

    pub fn is_symbol(self) -> bool {
        self.is_cell() && self.as_cell().is::<JsSymbol>()
    }

    pub fn as_object(self) -> Gc<JsObject> {
        assert!(self.is_object());
        unsafe { self.as_cell().downcast_unchecked() }
    }
    pub fn as_symbol(self) -> Gc<JsSymbol> {
        assert!(self.is_symbol());
        unsafe { self.as_cell().downcast_unchecked() }
    }
    pub fn get_primitive_proto(self, vm: &mut VirtualMachine) -> Gc<JsObject> {
        assert!(self.is_primitive());
        if self.is_string() {
            return vm.global_data().string_prototype.unwrap();
        } else if self.is_number() {
            return vm.global_data().number_prototype.unwrap();
        } else if self.is_boolean() {
            return vm.global_data().boolean_prototype.unwrap();
        } else {
            assert!(self.is_symbol());
            return vm.global_data().symbol_prototype.unwrap();
        }
    }

    pub fn get_slot(
        self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if !self.is_object() {
            if self.is_undefined_or_null() {
                let msg = JsString::new(vm, "null or undefined has no properties");
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            }
            if self.is_string() {
                let s = self.as_string();
                if name == Symbol::length() {
                    slot.set_1(
                        JsValue::new(s.len() as i32),
                        string_length(),
                        Some(s.as_dyn()),
                    );
                    return Ok(slot.value());
                }
            }

            let proto = self.get_primitive_proto(vm);
            if proto.get_property_slot(vm, name, slot) {
                return slot.get(vm, self);
            }
            return Ok(JsValue::undefined());
        }
        self.as_object().get_slot(vm, name, slot)
    }
    pub fn to_symbol(self, vm: &mut VirtualMachine) -> Result<Symbol, JsValue> {
        if self.is_number() {
            if self.is_int32() {
                if self.as_int32() as u32 as i32 == self.as_int32() {
                    return Ok(Symbol::Indexed(self.as_int32() as _));
                } else {
                    return Ok(vm.intern(self.as_int32().to_string()));
                }
            } else {
                let d = self.as_double();
                if d as u32 as f64 == d {
                    return Ok(Symbol::Indexed(d as u32));
                } else {
                    return Ok(vm.intern(d.to_string()));
                }
            }
        }

        if self.is_string() {
            return Ok(vm.intern(self.as_string().as_str()));
        }
        if self.is_symbol() {
            return Ok(self.as_symbol().sym());
        }
        if self.is_boolean() {
            if self.is_true() {
                return Ok(vm.intern("true"));
            } else {
                return Ok(vm.intern("false"));
            }
        }
        if self.is_null() {
            return Ok(Symbol::null());
        }

        if self.is_undefined() {
            return Ok(Symbol::undefined());
        }
        let mut obj = self.as_object();
        let prim = obj.to_primitive(vm, JsHint::String)?;
        prim.to_symbol(vm)
    }
    pub fn get_property_slot(
        self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<bool, JsValue> {
        let obj;
        if !self.is_object() {
            if self.is_undefined_or_null() {
                let msg = JsString::new(vm, "null or undefined has no properties");
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            }
            if self.is_string() {
                let s = self.as_string();
                if name == Symbol::length() {
                    slot.set_1(
                        JsValue::new(s.len() as i32),
                        string_length(),
                        Some(s.as_dyn()),
                    );
                    return Ok(true);
                }
            }
            obj = self.get_primitive_proto(vm);
        } else {
            obj = self.as_object();
        }
        Ok(obj.get_property_slot(vm, name, slot))
    }
    pub fn abstract_equal(self, other: JsValue, vm: &mut VirtualMachine) -> Result<bool, JsValue> {
        let mut lhs = self;
        let mut rhs = other;
        loop {
            if lhs.is_int32() && rhs.is_int32() {
                return Ok(lhs.as_int32() == rhs.as_int32());
            }

            if lhs.is_number() && rhs.is_number() {
                return Ok(lhs.number() == rhs.number());
            }
            if lhs.is_undefined_or_null() && rhs.is_undefined_or_null() {
                return Ok(true);
            }

            if lhs.is_string() && rhs.is_string() {
                return Ok(lhs.as_string().as_str() == rhs.as_string().as_str());
            }
            if lhs.is_object() && rhs.is_object() {
                return Ok(Gc::ptr_eq(self.as_object(), rhs.as_object()));
            }
            if lhs.is_symbol() && rhs.is_symbol() {
                return Ok((*rhs.as_symbol()) == (*rhs.as_symbol()));
            }

            // conversion phase
            if lhs.is_number() && rhs.is_string() {
                rhs = JsValue::new(rhs.to_number(vm)?);
                continue;
            }

            if lhs.is_string() && rhs.is_number() {
                lhs = JsValue::new(lhs.to_number(vm)?);
                continue;
            }

            if lhs.is_boolean() {
                lhs = JsValue::new(lhs.to_number(vm)?);
                continue;
            }

            if rhs.is_boolean() {
                rhs = JsValue::new(rhs.to_number(vm)?);
                continue;
            }

            if (lhs.is_string() || lhs.is_number()) && rhs.is_object() {
                rhs = rhs.to_primitive(vm, JsHint::None)?;
                continue;
            }
            if lhs.is_object() && (rhs.is_string() || rhs.is_number()) {
                lhs = lhs.to_primitive(vm, JsHint::None)?;
                continue;
            }
            break Ok(false);
        }
    }
    pub fn strict_equal(self, other: JsValue) -> bool {
        if self.is_int32() && other.is_int32() {
            return self.as_int32() == other.as_int32();
        }
        if self.is_number() && other.is_number() {
            return self.number() == other.number();
        }
        if !self.is_cell() || !other.is_cell() {
            return unsafe { self.u.as_int64 == other.u.as_int64 };
        }
        if self.is_cell() && other.is_cell() {
            match (
                self.as_cell().downcast::<JsString>(),
                other.as_cell().downcast::<JsString>(),
            ) {
                (Some(x), Some(y)) => return x.as_str() == y.as_str(),
                _ => (),
            }
        }

        unsafe { self.u.as_int64 == other.u.as_int64 }
    }
    pub fn same_value_impl(lhs: Self, rhs: Self, zero: bool) -> bool {
        if lhs.is_int32() {
            if rhs.is_int32() {
                return lhs.as_int32() == rhs.as_int32();
            }

            if zero && rhs.is_number() {
                return lhs.as_int32() as f64 == rhs.number();
            }
            // because +0(int32_t) and -0(double) is not the same value
            return false;
        } else if lhs.is_number() {
            if !rhs.is_number() {
                return false;
            }
            if !zero && rhs.is_int32() {
                return false;
            }

            let lhsn = lhs.number();
            let rhsn = rhs.number();
            if lhsn == rhsn {
                return true;
            }
            return lhsn.is_nan() && rhsn.is_nan();
        }

        if !lhs.is_cell() || !rhs.is_cell() {
            return unsafe { lhs.u.as_int64 == rhs.u.as_int64 };
        }
        if (lhs.is_cell() && lhs.as_cell().is::<JsString>())
            && (rhs.is_cell() && rhs.as_cell().is::<JsString>())
        {
            return unsafe {
                lhs.as_cell().downcast_unchecked::<JsString>().as_str()
                    == rhs.as_cell().downcast_unchecked::<JsString>().as_str()
            };
        }
        unsafe { lhs.u.as_int64 == rhs.u.as_int64 }
    }

    pub fn same_value(lhs: Self, rhs: Self) -> bool {
        Self::same_value_impl(lhs, rhs, false)
    }

    pub fn same_value_zero(lhs: Self, rhs: Self) -> bool {
        Self::same_value_impl(lhs, rhs, true)
    }
    pub fn is_any_int(self) -> bool {
        if self.is_int32() {
            return true;
        }
        if !self.is_number() {
            return false;
        }
        self.as_double() as i64 as f64 == self.as_double()
    }
    pub fn as_any_int(self) -> i64 {
        assert!(self.is_any_int());
        if self.is_int32() {
            return self.as_int32() as _;
        }
        self.as_double() as i64
    }
    pub fn to_boolean(self) -> bool {
        if self.is_number() {
            if self.is_int32() {
                self.as_int32() != 0
            } else {
                self.as_double() != 0.0 && !self.as_double().is_nan()
            }
        } else if self.is_cell() && self.as_cell().is::<JsString>() {
            self.as_cell().downcast::<JsString>().unwrap().len() != 0
        } else if self.is_undefined_or_null() {
            false
        } else if self.is_boolean() {
            self.as_boolean()
        } else {
            !self.is_empty()
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
    #[inline]
    pub fn compare(
        self,
        rhs: JsValue,
        left_first: bool,
        vm: &mut VirtualMachine,
    ) -> Result<i32, JsValue> {
        let lhs = self;
        if lhs.is_number() && rhs.is_number() {
            return Ok(Self::number_compare(self.number(), rhs.number()));
        }

        let px;
        let py;
        if left_first {
            px = lhs.to_primitive(vm, JsHint::Number)?;
            py = rhs.to_primitive(vm, JsHint::Number)?;
        } else {
            py = rhs.to_primitive(vm, JsHint::Number)?;
            px = lhs.to_primitive(vm, JsHint::Number)?;
        }

        if px.is_int32() && py.is_int32() {
            Ok(if px.as_int32() < py.as_int32() {
                CMP_TRUE
            } else {
                CMP_FALSE
            })
        } else if px.is_string() && py.is_string() {
            if px.as_string().as_str() < py.as_string().as_str() {
                Ok(CMP_TRUE)
            } else {
                Ok(CMP_FALSE)
            }
        } else {
            let nx = px.to_number(vm)?;
            let ny = py.to_number(vm)?;
            Ok(Self::number_compare(nx, ny))
        }
    }
}

impl From<bool> for JsValue {
    fn from(x: bool) -> Self {
        if x {
            Self::true_()
        } else {
            Self::false_()
        }
    }
}

impl<T: Cell + ?Sized> From<Gc<T>> for JsValue {
    fn from(x: Gc<T>) -> Self {
        Self {
            u: EncodedValueDescriptor { ptr: x.as_dyn() },
        }
    }
}

impl From<f64> for JsValue {
    fn from(d: f64) -> Self {
        if d as i32 as f64 == d {
            return Self::new(d as i32);
        }
        let int = unsafe { std::mem::transmute::<_, i64>(d) };
        Self {
            u: EncodedValueDescriptor {
                as_int64: int + Self::DOUBLE_ENCODE_OFFSET as i64,
            },
        }
    }
}

macro_rules! impl_from_int {
    ($($t: ty)*) => {
        $(
            impl From<$t> for JsValue {
                fn from(x: $t) -> Self {
                    let i = x as i32;
                    Self {
                        u: EncodedValueDescriptor {
                            as_int64: (Self::NUMBER_TAG as i64) | i as i64
                        }
                    }
                }
            }
        )*
    };
}

impl_from_int!(u8 i8 u16 i16 i32 u32);

/*pub trait JsValueNew<T> {
    #[allow(unused_variables)]
    fn new(value: T) -> JsValue {
        panic!();
    }
}


impl<T: Cell + ?Sized> JsValueNew<Heap<T>> for JsValue {
    fn new(value: Heap<T>) -> JsValue {
        Self {
            u: EncodedValueDescriptor {
                ptr: value.as_dyn(),
            },
        }
    }
}

impl JsValueNew<bool> for JsValue {
    fn new(value: bool) -> JsValue {
        if value {
            Self::true_()
        } else {
            Self::false_()
        }
    }
}
*/
impl PartialEq for JsValue {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.u.as_int64 == other.u.as_int64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_i32() {
        let val = JsValue::new(42i32);

        assert!(val.is_int32());
        assert_eq!(val.as_int32(), 42);
    }
    #[test]
    fn test_f64() {
        let val = JsValue::new(42.5);
        assert!(val.is_number() && val.is_double());
        assert_eq!(val.as_double(), 42.5);
    }
}

impl Cell for JsValue {}
unsafe impl Trace for JsValue {
    fn trace(&self, tracer: &mut dyn Tracer) {
        if self.is_cell() && !self.is_empty() {
            self.as_cell().trace(tracer);
        }
    }
}

impl Default for JsValue {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for JsValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.is_number() {
            serializer.serialize_f64(self.number())
        } else if self.is_boolean() {
            serializer.serialize_bool(self.is_true())
        } else if self.is_null() {
            "null".serialize(serializer)
        } else if self.is_undefined() {
            "undefined".serialize(serializer)
        } else {
            self.as_cell().serialize(serializer)
        }
    }
}
