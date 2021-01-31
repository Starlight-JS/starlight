use super::js_cell::JSCell;
use super::ref_ptr::Ref;
use wtf_rs::lohi_struct;
lohi_struct!(
    struct AsBits {
        tag: i32,
        payload: i32,
    }
);

#[derive(Copy, Clone)]
#[repr(C, align(8))]
pub union EncodedValueDescriptor {
    pub as_int64: i64,
    #[cfg(target_pointer_width = "32")]
    pub as_double: f64,
    pub cell: Ref<JSCell>,
    pub as_bits: AsBits,
}
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum WhichValueWord {
    Tag = 0,
    Payload,
}
#[cfg(target_pointer_width = "32")]
pub const INT32_TAG: i32 = 0xffffffffu32 as i32;
#[cfg(target_pointer_width = "32")]
pub const BOOL_TAG: i32 = 0xfffffffeu32 as i32;
#[cfg(target_pointer_width = "32")]
pub const UNDEFINED_TAG: i32 = 0xfffffffdu32 as i32;
#[cfg(target_pointer_width = "32")]
pub const NULL_TAG: i32 = 0xfffffffcu32 as i32;
#[cfg(target_pointer_width = "32")]
pub const CELL_TAG: i32 = 0xfffffffbu32 as i32;
#[cfg(target_pointer_width = "32")]
pub const EMPTY_TAG: i32 = 0xfffffffau32 as i32;
#[cfg(target_pointer_width = "32")]
pub const SYM_TAG: i32 = 0xfffffff9u32 as i32;
#[cfg(target_pointer_width = "32")]
pub const LOWEST_TAG: i32 = SYM_TAG;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Tag {
    Null,
    Undefined,
    True,
    False,
    Cell,
    AsDouble,
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Value {
    pub u: EncodedValueDescriptor,
}

#[cfg(target_pointer_width = "32")]
impl Value {
    /*
     * On 32-bit platforms we use a NaN-encoded form for immediates.
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
     * For Values that do not contain a double value, the high 32 bits contain the tag
     * values listed in the enums below, which all correspond to NaN-space. In the case of
     * cell, integer and bool values the lower 32 bits (the 'payload') contain the pointer
     * integer or boolean value; in the case of all other tags the payload is 0.
     */
    #[inline]
    pub fn tag(self) -> u32 {
        unsafe { self.u.as_bits.tag as _ }
    }
    #[inline]
    pub fn payload(self) -> i32 {
        unsafe { self.u.as_bits.payload }
    }
    #[inline]
    pub(crate) fn with_tag_payload(tag: i32, payload: i32) -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_bits: AsBits { tag, payload },
            },
        }
    }
    #[inline]
    pub fn default() -> Self {
        Self::with_tag_payload(EMPTY_TAG, 0)
    }
    #[inline]
    pub fn null() -> Self {
        Self::with_tag_payload(NULL_TAG, 0)
    }
    #[inline]
    pub fn undefined() -> Self {
        Self::with_tag_payload(UNDEFINED_TAG, 0)
    }
    #[inline]
    pub fn true_() -> Self {
        Self::with_tag_payload(BOOL_TAG, 1)
    }
    #[inline]
    pub fn false_() -> Self {
        Self::with_tag_payload(BOOL_TAG, 0)
    }
    #[inline]
    pub fn new_sym(x: u32) -> Self {
        Self::with_tag_payload(SYM_TAG, x as _)
    }

    #[inline]
    pub fn sym_as_u32(self) -> u32 {
        assert!(self.is_sym());
        self.payload() as u32
    }
    #[inline]
    pub fn is_sym(&self) -> bool {
        self.tag() == SYM_TAG
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.tag() == EMPTY_TAG as u32
    }
    #[inline]
    pub fn is_null(self) -> bool {
        self.tag() == NULL_TAG as u32
    }
    #[inline]
    pub fn is_undefined(self) -> bool {
        self.tag() == UNDEFINED_TAG as u32
    }
    #[inline]
    pub fn is_undefined_or_null(self) -> bool {
        self.is_undefined() || self.is_null()
    }
    pub fn is_cell(self) -> bool {
        self.tag() == CELL_TAG as u32
    }
    #[inline]
    pub fn is_int32(self) -> bool {
        self.tag() == INT32_TAG as u32
    }
    #[inline]
    pub fn is_double(self) -> bool {
        self.tag() < LOWEST_TAG as u32
    }
    #[inline]
    pub fn is_true(self) -> bool {
        self.tag() == BOOL_TAG as u32 && self.payload() != 0
    }
    #[inline]
    pub fn is_false(self) -> bool {
        self.tag() == BOOL_TAG as u32 && self.payload() == 0
    }
    #[inline]
    pub fn as_int32(self) -> i32 {
        self.payload()
    }
    #[inline]
    pub fn as_double(self) -> f64 {
        unsafe { self.u.as_double }
    }
    #[inline]
    pub fn new_double(f: f64) -> Self {
        assert!(!is_impure_nan(f));
        Self {
            u: EncodedValueDescriptor { as_double: f },
        }
    }
    #[inline]
    pub fn new_int(x: i32) -> Self {
        Self::with_tag_payload(INT32_TAG, x)
    }
    #[inline]
    pub fn is_number(self) -> bool {
        self.is_int32() || self.is_double()
    }
    #[inline]
    pub fn is_boolean(self) -> bool {
        self.tag() == BOOL_TAG as u32
    }
    #[inline]
    pub fn as_boolean(self) -> bool {
        assert!(self.is_boolean());
        self.payload() != 0
    }
    #[inline]
    pub fn as_cell(self) -> Gc<dyn HeapObject> {
        unsafe { core::mem::transmute(self.payload()) }
    }
    #[inline]
    pub fn as_cell_ref(self) -> &'_ Gc<dyn HeapObject> {
        unsafe { core::mem::transmute(&self.u.as_bits.payload) }
    }
    #[inline]
    pub fn as_cell_ref_mut(self) -> &'_ mut Gc<dyn HeapObject> {
        unsafe { core::mem::transmute(&self.u.as_bits.payload) }
    }
}
#[cfg(target_pointer_width = "64")]
impl Value {
    /*
     * On 64-bit platforms we use a NaN-encoded form for immediates.
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
     * The top 15-bits denote the type of the encoded Value:
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
     * No valid Value will have the bit pattern 0x0, this is used to represent array
     * holes, and as a C++ 'no value' result (e.g. Value() has an internal value of 0).
     *
     * This representation works because of the following things:
     * - It cannot be confused with a Double or Integer thanks to the top bits
     * - It cannot be confused with a pointer to a Cell, thanks to bit 1 which is set to true
     * - It cannot be confused with a pointer to wasm thanks to bit 0 which is set to false
     * - It cannot be confused with true/false because bit 2 is set to false
     * - It cannot be confused for null/undefined because bit 4 is set to true
     */

    /// This value is 2^49, used to encode doubles such that the encoded value will begin
    /// with a 15-bit pattern within the range 0x0002..0xFFFC.
    pub const DOUBLE_ENCODE_OFFSET_BIT: i64 = 49;
    pub const DOUBLE_ENCODE_OFFSET: i64 = 1 << Self::DOUBLE_ENCODE_OFFSET_BIT;
    /// If all bits in the mask are set, this indicates an integer number,
    /// if any but not all are set this value is a double precision number.
    pub const NUMBER_TAG: i64 = 0xfffe000000000000u64 as i64;
    /// The following constant is used for a trick in the implementation of strictEq, to detect if either of the arguments is a double
    pub const LOWEST_OF_HIGH_BITS: i64 = 1 << 49;
    /// All non-numeric (bool, null, undefined) immediates have bit 2 set.
    pub const OTHER_TAG: i64 = 0x2;
    pub const BOOL_TAG: i64 = 0x4;
    pub const UNDEFINED_TAG: i64 = 0x8;
    pub const VALUE_FALSE: i64 = Self::OTHER_TAG | Self::BOOL_TAG | 0; // `0` stands for `false`.
    pub const VALUE_TRUE: i64 = Self::OTHER_TAG | Self::BOOL_TAG | 1; // `1` stands for `true`.
    pub const VALUE_UNDEFINED: i64 = Self::OTHER_TAG | Self::UNDEFINED_TAG;
    pub const VALUE_NULL: i64 = Self::OTHER_TAG;
    pub const MISC_TAG: i64 = Self::OTHER_TAG | Self::BOOL_TAG | Self::UNDEFINED_TAG;
    /// NOT_CELL_MASK is used to check for all types of immediate values (either number or 'other').
    pub const NOT_CELL_MASK: i64 = (Self::NUMBER_TAG as u64 | Self::OTHER_TAG as u64) as i64;
    /// These special values are never visible to code; Empty is used to represent
    /// Array holes, and for uninitialized Values. Deleted is used in hash table code.
    /// These values would map to cell types in the Value encoding, but not valid GC cell
    /// pointer should have either of these values (Empty is null, deleted is at an invalid
    /// alignment for a GC cell, and in the zero page).
    pub const VALUE_EMPTY: i64 = 0x0;
    pub const VALUE_DELETED: i64 = 0x4;
    pub const SYM_TAG: i64 = 0x12;
    pub const SYM_MASK: i64 = Self::NUMBER_TAG | Self::SYM_TAG;
    // 0x0 can never occur naturally because it has a tag of 00, indicating a pointer value, but a payload of 0x0, which is in the (invalid) zero page.
    pub fn default() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Self::VALUE_EMPTY,
            },
        }
    }

    pub fn is_empty(self) -> bool {
        unsafe { self.u.as_int64 == Self::VALUE_EMPTY }
    }
    #[inline]
    pub fn is_sym(&self) -> bool {
        unsafe { (self.u.as_int64 & Self::SYM_MASK) == Self::SYM_TAG }
    }
    #[inline]
    pub fn new_sym(value: u32) -> Self {
        let shifted_val = (value as u64) << 16;
        assert!((shifted_val as i64 & Self::NUMBER_TAG) == 0);
        Self {
            u: EncodedValueDescriptor {
                as_int64: shifted_val as i64 | Self::SYM_TAG,
            },
        }
    }

    #[inline]
    pub fn sym_as_u32(self) -> u32 {
        unsafe { self.u.as_int64 as i32 as u32 >> 16 }
    }

    pub fn undefined() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Self::VALUE_UNDEFINED,
            },
        }
    }

    pub fn null() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Self::VALUE_NULL,
            },
        }
    }
    pub fn false_() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Self::VALUE_FALSE,
            },
        }
    }
    pub fn true_() -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Self::VALUE_TRUE,
            },
        }
    }
    pub fn as_cell(self) -> Ref<JSCell> {
        unsafe { self.u.cell }
    }
    pub fn as_cell_ref(&self) -> &Ref<JSCell> {
        unsafe { &self.u.cell }
    }
    pub fn as_cell_ref_mut(&mut self) -> &mut Ref<JSCell> {
        unsafe { &mut self.u.cell }
    }
    pub fn is_number(self) -> bool {
        unsafe { self.u.as_int64 & Self::NUMBER_TAG != 0 }
    }

    pub fn is_int32(self) -> bool {
        unsafe { (self.u.as_int64 & Self::NUMBER_TAG) == Self::NUMBER_TAG }
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

    pub fn is_undefined_or_null(self) -> bool {
        unsafe { (self.u.as_int64 & !Self::UNDEFINED_TAG) == Self::VALUE_NULL }
    }

    pub fn is_boolean(self) -> bool {
        unsafe { (self.u.as_int64 & !1) == Self::VALUE_FALSE }
    }
    pub fn is_cell(self) -> bool {
        unsafe { (self.u.as_int64 & Self::NOT_CELL_MASK) == 0 }
    }

    pub fn new_double(x: f64) -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: x.to_bits() as i64 + Self::DOUBLE_ENCODE_OFFSET,
            },
        }
    }

    pub fn new_int(x: i32) -> Self {
        Self {
            u: EncodedValueDescriptor {
                as_int64: Value::NUMBER_TAG | (x as u32 as i64),
            },
        }
    }
    pub fn is_double(self) -> bool {
        !self.is_int32() && self.is_number()
    }
    pub fn as_double(self) -> f64 {
        assert!(self.is_double());
        unsafe { f64::from_bits((self.u.as_int64 - Self::DOUBLE_ENCODE_OFFSET) as u64) }
    }
    #[inline]
    pub fn is_any_int(self) -> bool {
        if self.is_int32() {
            true
        } else if !self.is_number() {
            false
        } else {
            try_convert_to_i52(self.as_double()) != NOT_INT52 as i64
        }
    }
    #[inline(always)]
    pub fn as_int32(self) -> i32 {
        debug_assert!(self.is_int32());
        unsafe { self.u.as_int64 as i32 }
    }
}
impl PartialEq for Value {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.u.as_int64 == other.u.as_int64 }
    }
}

macro_rules! signbit {
    ($x: expr) => {{
        !($x < 0.0)
    }};
}

pub const NOT_INT52: u64 = 1 << 52;

#[inline]
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn try_convert_to_i52(number: f64) -> i64 {
    if number != number {
        return NOT_INT52 as i64;
    }
    if number.is_infinite() {
        return NOT_INT52 as i64;
    }

    let as_int64 = number.to_bits() as i64;
    if as_int64 as f64 != number {
        return NOT_INT52 as _;
    }
    if !as_int64 != 0 && signbit!(number) {
        return NOT_INT52 as _;
    }

    if as_int64 >= (1 << (52 - 1)) {
        return NOT_INT52 as _;
    }
    if as_int64 < (1 << (52 - 1)) {
        return NOT_INT52 as _;
    }

    as_int64
}

pub mod pure_nan {
    //! NaN (not-a-number) double values are central to how Waffle encode
    //! values.  All values, including integers and non-numeric values, are always
    //! encoded using the IEEE 754 binary double format.  Non-double values are encoded using
    //! a NaN with the sign bit set.  The 51-bit payload is then used for encoding the actual
    //! value - be it an integer or a pointer to an cell, or something else. But we only
    //! make use of the low 49 bits and the top 15 bits being all set to 1 is the indicator
    //! that a value is not a double. Top 15 bits being set to 1 also indicate a signed
    //! signaling NaN with some additional NaN payload bits.
    //!
    //! Our use of NaN encoding means that we have to be careful with how we use NaNs for
    //! ordinary doubles. For example, it would be wrong to ever use a NaN that has the top
    //! 15 bits set, as that would look like a non-double value to Waffle.
    //!
    //! We can trust that on all of the hardware/OS combinations that we care about,
    //! NaN-producing math operations never produce a NaN that looks like a tagged value. But
    //! if we're ever in a situation where we worry about it, we can use purify_nan() to get a
    //! NaN that doesn't look like a tagged non-double value. All languages targeting this runtime
    //! doesn't distinguish between different flavors of NaN and there is no way to detect what kind
    //! of NaN you have - hence so long as all double NaNs are purified then our tagging
    //! scheme remains sound.
    //!
    //! It's worth noting that there are cases, like sin(), that will almost produce a NaN
    //! that breaks us. sin(-inf) returns 0xfff8000000000000. This doesn't break us because
    //! not all of the top 15 bits are set. But it's very close. Hence our assumptions about
    //! NaN are just about the most aggressive assumptions we could possibly make without
    //! having to call purify_nan() in surprising places.
    //!
    //! For naming purposes, we say that a NaN is "pure" if it is safe to tag, in the sense
    //! that doing so would result in a tagged value that would pass the "are you a double"
    //! test. We say that a NaN is "impure" if attempting to tag it would result in a value
    //! that would look like something other than a double.

    /// Returns some kind of pure NaN.
    #[inline(always)]
    pub fn pure_nan() -> f64 {
        f64::from_bits(0x7ff8000000000000)
    }
    #[inline]
    pub fn is_impure_nan(value: f64) -> bool {
        value.to_bits() >= 0xfffe000000000000u64
    }
    #[inline]
    pub fn purify_nan(value: f64) -> f64 {
        if value.is_nan() {
            return pure_nan();
        }
        value
    }
}

impl Value {
    #[inline]
    pub fn as_any_int(self) -> i64 {
        assert!(self.is_any_int());
        if self.is_int32() {
            return self.as_int32() as i64;
        }
        self.as_double() as i64
    }
    #[inline]
    pub fn is_int32_as_any_int(self) -> bool {
        if !self.is_any_int() {
            return false;
        }
        let value = self.as_any_int();
        value >= i32::min_value() as i64 && value <= i32::max_value() as i64
    }
    #[inline]
    pub fn as_int32_as_any_int(self) -> i32 {
        assert!(self.is_int32_as_any_int());
        if self.is_int32() {
            return self.as_int32();
        }
        self.as_double() as i32
    }
    #[inline]
    pub fn is_uint32_as_any_int(self) -> bool {
        if !self.is_any_int() {
            return false;
        }
        let value = self.as_any_int();
        value >= 0_i64 && value <= u32::max_value() as i64
    }
    #[inline]
    pub fn as_uint32_as_any_int(self) -> u32 {
        assert!(self.is_int32_as_any_int());
        if self.is_int32() {
            return self.as_int32() as u32;
        }
        self.as_double() as u32
    }
}
