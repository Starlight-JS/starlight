//! NaN (not-a-number) double values are central to how JavaScriptCore encodes JavaScript
//! values (JSValues).  All values, including integers and non-numeric values, are always
//! encoded using the IEEE 754 binary double format.  Non-double values are encoded using
//! a NaN with the sign bit set.  The 51-bit payload is then used for encoding the actual
//! value - be it an integer or a pointer to an object, or something else. But we only
//! make use of the low 49 bits and the top 15 bits being all set to 1 is the indicator
//! that a value is not a double. Top 15 bits being set to 1 also indicate a signed
//! signaling NaN with some additional NaN payload bits.
//!
//! Our use of NaN encoding means that we have to be careful with how we use NaNs for
//! ordinary doubles. For example, it would be wrong to ever use a NaN that has the top
//! 15 bits set, as that would look like a non-double value to JSC.
//!
//! We can trust that on all of the hardware/OS combinations that we care about,
//! NaN-producing math operations never produce a NaN that looks like a tagged value. But
//! if we're ever in a situation where we worry about it, we can use purifyNaN() to get a
//! NaN that doesn't look like a tagged non-double value. The JavaScript language doesn't
//! distinguish between different flavors of NaN and there is no way to detect what kind
//! of NaN you have - hence so long as all double NaNs are purified then our tagging
//! scheme remains sound.
//!
//! It's worth noting that there are cases, like sin(), that will almost produce a NaN
//! that breaks us. sin(-inf) returns 0xfff8000000000000. This doesn't break us because
//! not all of the top 15 bits are set. But it's very close. Hence our assumptions about
//! NaN are just about the most aggressive assumptions we could possibly make without
//! having to call purifyNaN() in surprising places.
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
    // Tests if the double value would break JSVALUE64 encoding, which is the most
    // aggressive kind of encoding that we currently use.
    value.to_bits() >= 0xfffe000000000000
}
/// If the given value is NaN then return a NaN that is known to be pure.
pub fn purify_nan(value: f64) -> f64 {
    if value != value {
        return pure_nan();
    }
    value
}
