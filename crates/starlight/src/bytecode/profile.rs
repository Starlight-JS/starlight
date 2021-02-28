use crate::vm::value::JsValue;

#[derive(Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct ObservedType {
    bits: u8,
}

impl ObservedType {
    pub const fn default() -> Self {
        Self { bits: 0 }
    }
    pub const TYPE_EMPTY: u8 = 0x0;
    pub const TYPE_INT32: u8 = 0x1;
    pub const TYPE_NUMBER: u8 = 0x2;
    pub const TYPE_NON_NUMBER: u8 = 0x04;
    pub const fn bits(self) -> u8 {
        self.bits
    }
    pub const fn saw_int32(self) -> bool {
        (self.bits & Self::TYPE_INT32) != 0
    }

    pub const fn is_only_int32(self) -> bool {
        self.bits == Self::TYPE_INT32
    }

    pub const fn saw_number(self) -> bool {
        (self.bits & Self::TYPE_NUMBER) != 0
    }

    pub const fn is_only_number(self) -> bool {
        self.bits == Self::TYPE_NUMBER
    }

    pub const fn saw_non_number(self) -> bool {
        (self.bits & Self::TYPE_NON_NUMBER) != 0
    }

    pub const fn is_only_non_number(self) -> bool {
        self.bits == Self::TYPE_NON_NUMBER
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub const fn with_int32(self) -> Self {
        Self {
            bits: self.bits | Self::TYPE_INT32,
        }
    }

    pub const fn with_number(self) -> Self {
        Self {
            bits: self.bits | Self::TYPE_NUMBER,
        }
    }

    pub const fn with_non_number(self) -> Self {
        Self {
            bits: self.bits | Self::TYPE_NON_NUMBER,
        }
    }

    pub const fn without_non_number(self) -> Self {
        Self {
            bits: self.bits & !Self::TYPE_NON_NUMBER,
        }
    }

    pub const fn new(bits: u8) -> Self {
        Self { bits }
    }
}

#[rustfmt::skip]
pub enum ResultsTag {
    NonNegZeroDouble = 1 << 0,
    NegZeroDouble    = 1 << 1,
    NonNumeric       = 1 << 2,
    Int32Overflow    = 1 << 3,
    HeapBigInt       = 1 << 4,
}
use ResultsTag::*;
#[derive(Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObservedResults {
    bits: u8,
}

impl ObservedResults {
    pub const NUM_BITS_NEEDED: u8 = 5;
    pub const fn new(bits: u8) -> Self {
        Self { bits }
    }

    pub const fn did_observe_not_int32(self) -> bool {
        (self.bits
            & (NonNegZeroDouble as u8 | NegZeroDouble as u8 | NonNumeric as u8 | HeapBigInt as u8))
            != 0
    }

    pub const fn did_observe_double(self) -> bool {
        (self.bits & (NonNegZeroDouble as u8 | NegZeroDouble as u8)) != 0
    }

    pub const fn did_observe_neg_zero_double(self) -> bool {
        (self.bits & (NegZeroDouble as u8)) != 0
    }

    pub const fn did_observe_non_neg_zero_double(self) -> bool {
        (self.bits & (NonNegZeroDouble as u8)) != 0
    }

    pub const fn did_observe_non_numeric(self) -> bool {
        (self.bits & (NonNumeric as u8)) != 0
    }

    pub const fn did_observe_heap_bigint(self) -> bool {
        (self.bits & (HeapBigInt as u8)) != 0
    }

    pub const fn did_observe_int32_overflow(self) -> bool {
        (self.bits & (Int32Overflow as u8)) != 0
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum ArithProfile {
    ///- ObservedResults
    ///- ObservedType for right-hand-side
    /// - ObservedType for left-hand-side
    /// - a bit used by division to indicate whether a special fast path was taken
    Binary(u16),
    /// - ObservedResults
    /// - ObservedType for the argument
    Unary(u16),
}

impl ArithProfile {
    pub fn should_emit_set_double(&self) -> bool {
        let mask = Int32Overflow as u16 | NegZeroDouble as u16 | NonNegZeroDouble as u16;
        (self.bits() & mask) != 0
    }

    pub fn should_emit_set_non_numeric(&self) -> bool {
        (self.bits() & NonNumeric as u16) != 0
    }

    pub fn should_emit_set_heap_bigint(&self) -> bool {
        (self.bits() & HeapBigInt as u16) != 0
    }

    pub fn bits(self) -> u16 {
        match self {
            ArithProfile::Binary(x) => x,
            ArithProfile::Unary(x) => x,
        }
    }
    pub fn bits_ref(&self) -> &u16 {
        match self {
            ArithProfile::Binary(x) => x,
            ArithProfile::Unary(x) => x,
        }
    }
    pub fn bits_ref_mut(&mut self) -> &mut u16 {
        match self {
            ArithProfile::Binary(x) => x,
            ArithProfile::Unary(x) => x,
        }
    }
    pub fn observed_results(self) -> ObservedResults {
        ObservedResults::new(
            (self.bits() & ((1 << ObservedResults::NUM_BITS_NEEDED as u16) - 1)) as u8,
        )
    }

    pub fn did_observe_not_int32(self) -> bool {
        self.observed_results().did_observe_not_int32()
    }

    pub fn did_observe_double(self) -> bool {
        self.observed_results().did_observe_double()
    }

    pub fn did_observe_heap_bigint(self) -> bool {
        self.observed_results().did_observe_heap_bigint()
    }

    pub fn did_observe_int32_overflow(self) -> bool {
        self.observed_results().did_observe_int32_overflow()
    }

    pub fn did_observe_non_numeric(self) -> bool {
        self.observed_results().did_observe_non_numeric()
    }

    pub fn did_observe_non_neg_zero_double(self) -> bool {
        self.observed_results().did_observe_non_neg_zero_double()
    }

    pub fn did_observe_neg_zero_double(self) -> bool {
        self.observed_results().did_observe_neg_zero_double()
    }
    pub fn set_observed_non_neg_zero_double(&mut self) {
        self.set_bit(NonNegZeroDouble as _);
    }

    pub fn set_observed_neg_zero_double(&mut self) {
        self.set_bit(NegZeroDouble as _);
    }

    pub fn set_observed_non_numeric(&mut self) {
        self.set_bit(NonNumeric as _);
    }

    pub fn set_observed_heap_bigint(&mut self) {
        self.set_bit(HeapBigInt as _);
    }

    pub fn set_observed_int32_overflow(&mut self) {
        self.set_bit(Int32Overflow as _);
    }

    pub fn observe_result(&mut self, val: JsValue) {
        if val.is_number() {
            *self.bits_ref_mut() |=
                Int32Overflow as u16 | NonNegZeroDouble as u16 | NegZeroDouble as u16;
        }

        *self.bits_ref_mut() |= NonNumeric as u16;
    }

    pub fn has_bits(self, mask: u16) -> bool {
        (self.bits() & mask) != 0
    }

    pub fn set_bit(&mut self, mask: u16) {
        *self.bits_ref_mut() |= mask;
    }

    pub fn is_unary(&self) -> bool {
        match self {
            Self::Unary(_) => true,
            _ => false,
        }
    }

    pub fn is_binary(&self) -> bool {
        !self.is_unary()
    }
}

pub const ARG_OBSERVED_TYPE_SHIFT: u16 = ObservedResults::NUM_BITS_NEEDED as _;
pub const CLEAR_ARG_OBSERVED_TYPE_BIT_MASK: u16 = !(0b111 << ARG_OBSERVED_TYPE_SHIFT);
pub const OBSERVED_TYPE_MASK: u16 = (1 << ObservedResults::NUM_BITS_NEEDED as u16) - 1;
// impl ArithProfile::Unary
impl ArithProfile {
    pub const fn unary_observed_int_bits() -> Self {
        Self::Unary((ObservedType::default().with_int32().bits() as u16) << ARG_OBSERVED_TYPE_SHIFT)
    }

    pub const fn unary_observed_num_bits() -> Self {
        Self::Unary(
            (ObservedType::default().with_number().bits() as u16) << ARG_OBSERVED_TYPE_SHIFT,
        )
    }

    pub fn unary_arg_observed_type(self) -> ObservedType {
        debug_assert!(self.is_unary());
        return ObservedType::new(
            ((self.bits() >> ARG_OBSERVED_TYPE_SHIFT) & OBSERVED_TYPE_MASK) as u8,
        );
    }
    pub fn unary_set_arg_observed_type(&mut self, ty: ObservedType) {
        debug_assert!(self.is_unary());
        let mut bits = self.bits();
        bits &= CLEAR_ARG_OBSERVED_TYPE_BIT_MASK;
        bits |= (ty.bits() as u16) << ARG_OBSERVED_TYPE_SHIFT;
        *self.bits_ref_mut() = bits;
        debug_assert_eq!(self.unary_arg_observed_type(), ty);
    }

    pub fn unary_arg_saw_int32(&mut self) {
        self.unary_set_arg_observed_type(self.unary_arg_observed_type().with_int32())
    }
    pub fn unary_arg_saw_number(&mut self) {
        self.unary_set_arg_observed_type(self.unary_arg_observed_type().with_number())
    }
    pub fn unary_arg_saw_non_number(&mut self) {
        self.unary_set_arg_observed_type(self.unary_arg_observed_type().with_non_number())
    }

    pub fn observe_unary_arg(&mut self, arg: JsValue) {
        let mut new_profile = *self;
        if arg.is_number() {
            if arg.is_int32() {
                new_profile.unary_arg_saw_int32();
            } else {
                new_profile.unary_arg_saw_number();
            }
        } else {
            new_profile.unary_arg_saw_non_number();
        }
        *self.bits_ref_mut() = new_profile.bits();
    }

    pub fn is_unary_observed_type_empty(self) -> bool {
        self.unary_arg_observed_type().is_empty()
    }
}

pub const RHS_OBSERVED_TYPE_SHIFT: u16 = ObservedResults::NUM_BITS_NEEDED as _;
pub const LHS_OBSERVED_TYPE_SHIFT: u16 = RHS_OBSERVED_TYPE_SHIFT + RHS_OBSERVED_TYPE_SHIFT;

pub const CLEAR_RHS_OBSERVED_TYPE_MASK: u16 = !(0b111 << RHS_OBSERVED_TYPE_SHIFT);
pub const CLEAR_LHS_OBSERVED_TYPE_MASK: u16 = !(0b111 << LHS_OBSERVED_TYPE_SHIFT);
pub const OBSERVED_TYPE_MASK_BINARY: u16 = (1 << 3) - 1;
pub const SPECIAL_FAST_PATH_BIT: u16 = 1 << (LHS_OBSERVED_TYPE_SHIFT + 3);

// impl ArithProfile::Binary
impl ArithProfile {
    pub fn observed_int_int_bits() -> Self {
        const OI: ObservedType = ObservedType::default().with_int32();
        Self::Binary(
            (OI.bits() as u16) << LHS_OBSERVED_TYPE_SHIFT
                | ((OI.bits() as u16) << RHS_OBSERVED_TYPE_SHIFT),
        )
    }

    pub fn observed_num_int_bits() -> Self {
        const OI: ObservedType = ObservedType::default().with_int32();
        const ON: ObservedType = ObservedType::default().with_number();
        Self::Binary(
            (ON.bits() as u16) << LHS_OBSERVED_TYPE_SHIFT
                | ((OI.bits() as u16) << RHS_OBSERVED_TYPE_SHIFT),
        )
    }
    pub fn observed_int_num_bits() -> Self {
        const OI: ObservedType = ObservedType::default().with_int32();
        const ON: ObservedType = ObservedType::default().with_number();
        Self::Binary(
            (OI.bits() as u16) << LHS_OBSERVED_TYPE_SHIFT
                | ((ON.bits() as u16) << RHS_OBSERVED_TYPE_SHIFT),
        )
    }

    pub fn observed_num_num_bits() -> Self {
        const ON: ObservedType = ObservedType::default().with_number();
        Self::Binary(
            (ON.bits() as u16) << LHS_OBSERVED_TYPE_SHIFT
                | ((ON.bits() as u16) << RHS_OBSERVED_TYPE_SHIFT),
        )
    }

    pub fn lhs_observed_type(self) -> ObservedType {
        ObservedType::new((self.bits() >> LHS_OBSERVED_TYPE_SHIFT & OBSERVED_TYPE_MASK) as u8)
    }
    pub fn rhs_observed_type(self) -> ObservedType {
        ObservedType::new((self.bits() >> RHS_OBSERVED_TYPE_SHIFT & OBSERVED_TYPE_MASK) as u8)
    }

    pub fn set_lhs_observed_type(&mut self, ty: ObservedType) {
        let mut bits = self.bits();
        bits &= CLEAR_LHS_OBSERVED_TYPE_MASK;
        bits |= (ty.bits() as u16) << LHS_OBSERVED_TYPE_SHIFT;
        *self.bits_ref_mut() = bits;
        debug_assert_eq!(self.lhs_observed_type(), ty);
    }
    pub fn set_rhs_observed_type(&mut self, ty: ObservedType) {
        let mut bits = self.bits();
        bits &= CLEAR_RHS_OBSERVED_TYPE_MASK;
        bits |= (ty.bits() as u16) << RHS_OBSERVED_TYPE_SHIFT;
        *self.bits_ref_mut() = bits;
        debug_assert_eq!(self.rhs_observed_type(), ty);
    }

    pub fn took_special_fast_path(self) -> bool {
        (self.bits() & SPECIAL_FAST_PATH_BIT) != 0
    }

    pub fn lhs_saw_int32(&mut self) {
        self.set_lhs_observed_type(self.lhs_observed_type().with_int32())
    }
    pub fn lhs_saw_number(&mut self) {
        self.set_lhs_observed_type(self.lhs_observed_type().with_number())
    }
    pub fn lhs_saw_non_number(&mut self) {
        self.set_lhs_observed_type(self.lhs_observed_type().with_non_number())
    }

    pub fn rhs_saw_int32(&mut self) {
        self.set_rhs_observed_type(self.rhs_observed_type().with_int32())
    }
    pub fn rhs_saw_number(&mut self) {
        self.set_rhs_observed_type(self.rhs_observed_type().with_number())
    }
    pub fn rhs_saw_non_number(&mut self) {
        self.set_rhs_observed_type(self.rhs_observed_type().with_non_number())
    }

    pub fn observe_lhs(&mut self, val: JsValue) {
        let mut new_profile = *self;
        if val.is_number() {
            if val.is_int32() {
                new_profile.lhs_saw_int32();
            } else {
                new_profile.lhs_saw_number();
            }
        } else {
            new_profile.lhs_saw_non_number();
        }
        *self.bits_ref_mut() = new_profile.bits();
    }
    pub fn observe_rhs(&mut self, val: JsValue) {
        let mut new_profile = *self;
        if val.is_number() {
            if val.is_int32() {
                new_profile.rhs_saw_int32();
            } else {
                new_profile.rhs_saw_number();
            }
        } else {
            new_profile.rhs_saw_non_number();
        }
        *self.bits_ref_mut() = new_profile.bits();
    }

    pub fn observe_lhs_and_rhs(&mut self, lhs: JsValue, rhs: JsValue) {
        self.observe_lhs(lhs);
        self.observe_rhs(rhs);
    }

    pub fn is_observed_type_empty(self) -> bool {
        self.lhs_observed_type().is_empty() && self.rhs_observed_type().is_empty()
    }
}
