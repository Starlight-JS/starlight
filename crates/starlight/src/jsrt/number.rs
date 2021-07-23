use num::traits::float::FloatCore;

use crate::{
    constant::S_CONSTURCTOR,
    prelude::*,
    vm::{context::Context, number::NumberObject},
};
pub fn number_value_of(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    if !obj.is_number() {
        if obj.is_jsobject() && obj.get_jsobject().is_class(NumberObject::class()) {
            return Ok(JsValue::new(
                NumberObject::to_ref(&obj.get_jsobject()).get(),
            ));
        } else {
            return Err(JsValue::new(ctx.new_type_error(
                "Number.prototype.valueOf is not generic function",
            )));
        }
    } else {
        Ok(obj)
    }
}
pub fn number_clz(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    let x = if !obj.is_number() {
        if obj.is_jsobject() && obj.get_jsobject().is_class(NumberObject::class()) {
            NumberObject::to_ref(&obj.get_jsobject()).get() as u32
        } else {
            return Err(JsValue::new(ctx.new_type_error(
                "Number.prototype.valueOf is not generic function",
            )));
        }
    } else {
        obj.get_number() as u32
    };
    Ok(JsValue::new(x.leading_zeros()))
}

pub fn number_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.ctor_call {
        let mut res = 0.0;
        if args.size() != 0 {
            res = args.at(0).to_number(ctx)?;
        }
        Ok(JsValue::new(NumberObject::new(ctx, res)))
    } else if args.size() == 0 {
        return Ok(JsValue::new(0i32));
    } else {
        return args.at(0).to_number(ctx).map(JsValue::new);
    }
}
pub fn number_is_nan(_ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let num = args.at(0);
    if !num.is_number() {
        return Ok(JsValue::new(false));
    }
    Ok(JsValue::new(num.get_number().is_nan()))
}

pub fn number_is_finite(_ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let num = args.at(0);
    if !num.is_number() {
        return Ok(JsValue::new(false));
    }
    Ok(JsValue::new(num.get_number().is_finite()))
}

fn this_number_val(ctx: GcPointer<Context>, obj: JsValue) -> Result<f64, JsValue> {
    let num;
    if !obj.is_number() {
        if obj.is_jsobject() && obj.get_jsobject().is_class(NumberObject::class()) {
            num = NumberObject::to_ref(&obj.get_jsobject()).get();
        } else {
            return Err(JsValue::new(ctx.new_type_error(
                "Number.prototype.toString is not generic function",
            )));
        }
    } else {
        num = obj.get_number();
    }

    Ok(num)
}

pub fn number_is_integer(_ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let num = args.at(0);
    if !num.is_number() {
        return Ok(JsValue::new(false));
    }
    Ok(JsValue::new(
        num.get_number() as i32 as f64 == num.get_number(),
    ))
}

pub fn number_to_int(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let num = args.at(0);
    num.to_int32(ctx).map(JsValue::new)
}
pub fn number_to_precisiion(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let precision_var = args.at(0);
    let mut this_num = this_number_val(ctx, args.this)?;
    if precision_var.is_undefined() || !this_num.is_finite() {
        return number_to_string(ctx, &Arguments::new(args.this, &mut []));
    }

    let precision = match precision_var.to_int32(ctx)? {
        x if (1..=100).contains(&x) => x as usize,
        _ => {
            let msg = JsString::new(
                ctx,
                "precision must be an integer in range between 1 and 100",
            );
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }
    };

    let precision_i32 = precision as i32;

    // 7
    let mut prefix = String::new(); // spec: 's'
    let mut suffix: String; // spec: 'm'
    let exponent: i32; // spec: 'e'

    // 8
    if this_num < 0.0 {
        prefix.push('-');
        this_num = -this_num;
    }

    // 9
    if this_num == 0.0 {
        suffix = "0".repeat(precision);
        exponent = 0;
    // 10
    } else {
        // Due to f64 limitations, this pactx differs a bit from the spec,
        // but has the same effect. It manipulates the string constructed
        // by ryu-js: digits with an optional dot between two of them.

        let mut buffer = ryu_js::Buffer::new();
        suffix = buffer.format(this_num).to_string();

        // a: getting an exponent
        exponent = flt_str_to_exp(&suffix);
        // b: getting relevant digits only
        if exponent < 0 {
            suffix = suffix.split_off((1 - exponent) as usize);
        } else if let Some(n) = suffix.find('.') {
            suffix.remove(n);
        }
        // impl: having exactly `precision` digits in `suffix`
        suffix = round_to_precision(&mut suffix, precision);

        // c: switching to scientific notation
        let great_exp = exponent >= precision_i32;
        if exponent < -6 || great_exp {
            // ii
            if precision > 1 {
                suffix.insert(1, '.');
            }
            // vi
            suffix.push('e');
            // iii
            if great_exp {
                suffix.push('+');
            }
            // iv, v
            suffix.push_str(&exponent.to_string());

            return Ok(JsValue::new(JsString::new(ctx, prefix + &suffix)));
        }
    } // 11
    let e_inc = exponent + 1;
    if e_inc == precision_i32 {
        return Ok(JsValue::from(JsString::new(ctx, prefix + &suffix)));
    }

    // 12
    if exponent >= 0 {
        suffix.insert(e_inc as usize, '.');
    // 13
    } else {
        prefix.push('0');
        prefix.push('.');
        prefix.push_str(&"0".repeat(-e_inc as usize));
    }

    // 14
    Ok(JsValue::new(JsString::new(ctx, prefix + &suffix)))
}

pub fn number_to_fixed(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let fixed_var = args.at(0);
    let mut this_num = this_number_val(ctx, args.this)?;

    let mut fixed = 0;
    if !fixed_var.is_undefined() {
        fixed = fixed_var.to_int32(ctx)?;
    }

    if !(0..=20).contains(&fixed) {
        let msg = JsString::new(ctx, "toFixed() digits argument must be between 0 and 20");
        return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
    }

    let fixed = fixed as usize;

    let mut buffer = ryu_js::Buffer::new();
    let mut string = buffer.format(this_num).to_string();
    // after .
    string = round_to_fixed(&mut string, fixed);
    Ok(JsValue::new(JsString::new(ctx, string)))
}

pub fn number_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let obj = args.this;
    let num;
    if !obj.is_number() {
        if obj.is_jsobject() && obj.get_jsobject().is_class(NumberObject::class()) {
            num = NumberObject::to_ref(&obj.get_jsobject()).get();
        } else {
            return Err(JsValue::new(ctx.new_type_error(
                "Number.prototype.toString is not generic function",
            )));
        }
    } else {
        num = obj.get_number();
    }

    if args.size() != 0 {
        let first = args.at(0);
        let radix;
        if first.is_undefined() {
            radix = 10;
        } else {
            radix = first.to_int32(ctx)?;
        }
        if radix == 10 {
            return Ok(JsValue::new(JsString::new(ctx, num.to_string())));
        }
        if (2..=36).contains(&radix) {
            if radix != 10 {
                if num.is_nan() {
                    return Ok(JsValue::new(JsString::new(ctx, "NaN")));
                } else if num.is_infinite() {
                    if num.is_sign_positive() {
                        return Ok(JsValue::new(JsString::new(ctx, "Infinity")));
                    } else {
                        return Ok(JsValue::new(JsString::new(ctx, "-Infinity")));
                    }
                }
            }
            Ok(JsValue::new(JsString::new(
                ctx,
                to_native_string_radix(num, radix as _),
            )))
        } else {
            let msg = JsString::new(ctx, "Illegal radix");
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }
    } else {
        return Ok(JsValue::new(JsString::new(ctx, num.to_string())));
    }
}

// https://chromium.googlesource.com/v8/v8/+/refs/heads/master/src/numbers/conversions.cc#1230
#[allow(clippy::wrong_self_convention)]
pub(crate) fn to_native_string_radix(mut value: f64, radix: u8) -> String {
    assert!(radix >= 2);
    assert!(radix <= 36);
    assert!(value.is_finite());
    // assectx_ne!(0.0, value);

    // Character array used for conversion.
    // Temporary buffer for the result. We start with the decimal point in the
    // middle and write to the left for the integer pactx and to the right for the
    // fractional pactx. 1024 characters for the exponent and 52 for the mantissa
    // either way, with additional space for sign, decimal point and string
    // termination should be sufficient.
    let mut buffer: [u8; BUF_SIZE] = [0; BUF_SIZE];
    let (int_buf, frac_buf) = buffer.split_at_mut(BUF_SIZE / 2);
    let mut fraction_cursor = 0;
    let negative = value.is_sign_negative();
    if negative {
        value = -value
    }
    // Split the value into an integer pactx and a fractional pactx.
    // let mut integer = value.trunc();
    // let mut fraction = value.fract();
    let mut integer = value.floor();
    let mut fraction = value - integer;

    // We only compute fractional digits up to the input double's precision.
    let mut delta = 0.5 * (next_after(value, f64::MAX) - value);
    delta = next_after(0.0, f64::MAX).max(delta);
    assert!(delta > 0.0);
    if fraction >= delta {
        // Insectx decimal point.
        frac_buf[fraction_cursor] = b'.';
        fraction_cursor += 1;
        loop {
            // Shift up by one digit.
            fraction *= radix as f64;
            delta *= radix as f64;
            // Write digit.
            let digit = fraction as u32;
            frac_buf[fraction_cursor] = std::char::from_digit(digit, radix as u32).unwrap() as u8;
            fraction_cursor += 1;
            // Calculate remainder.
            fraction -= digit as f64;
            // Round to even.
            if fraction + delta > 1.0
                && (fraction > 0.5 || (fraction - 0.5).abs() < f64::EPSILON && digit & 1 != 0)
            {
                loop {
                    // We need to back trace already written digits in case of carry-over.
                    fraction_cursor -= 1;
                    if fraction_cursor == 0 {
                        //              CHECK_EQ('.', buffer[fraction_cursor]);
                        // Carry over to the integer pactx.
                        integer += 1.;
                        break;
                    } else {
                        let c: u8 = frac_buf[fraction_cursor];
                        // Reconstruct digit.
                        let digit_0 = (c as char).to_digit(10).unwrap();
                        if digit_0 + 1 >= radix as u32 {
                            continue;
                        }
                        frac_buf[fraction_cursor] =
                            std::char::from_digit(digit_0 + 1, radix as u32).unwrap() as u8;
                        fraction_cursor += 1;
                        break;
                    }
                }
                break;
            }
            if fraction < delta {
                break;
            }
        }
    }

    // Compute integer digits. Fill unrepresented digits with zero.
    let mut int_iter = int_buf.iter_mut().enumerate().rev(); //.rev();
    while FloatCore::integer_decode(integer / f64::from(radix)).1 > 0 {
        integer /= radix as f64;
        *int_iter.next().unwrap().1 = b'0';
    }

    loop {
        let remainder = integer % (radix as f64);
        *int_iter.next().unwrap().1 =
            std::char::from_digit(remainder as u32, radix as u32).unwrap() as u8;
        integer = (integer - remainder) / radix as f64;
        if integer <= 0f64 {
            break;
        }
    }
    // Add sign and terminate string.
    if negative {
        *int_iter.next().unwrap().1 = b'-';
    }
    assert!(fraction_cursor < BUF_SIZE);

    let integer_cursor = int_iter.next().unwrap().0 + 1;
    let fraction_cursor = fraction_cursor + BUF_SIZE / 2;
    // dbg!("Number: {}, Radix: {}, Cursors: {}, {}", value, radix, integer_cursor, fraction_cursor);
    String::from_utf8_lossy(&buffer[integer_cursor..fraction_cursor]).into()
}
const BUF_SIZE: usize = 2200;
// https://golang.org/src/math/nextafter.go
#[inline]
fn next_after(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        f64::NAN
    } else if (x - y) == 0. {
        x
    } else if x == 0.0 {
        f64::from_bits(1).copysign(y)
    } else if y > x || x > 0.0 {
        f64::from_bits(x.to_bits() + 1)
    } else {
        f64::from_bits(x.to_bits() - 1)
    }
}
/// flt_str_to_exp - used in to_precision
///
/// This function traverses a string representing a number,
/// returning the floored log10 of this number.
///
fn flt_str_to_exp(flt: &str) -> i32 {
    let mut non_zero_encountered = false;
    let mut dot_encountered = false;
    for (i, c) in flt.chars().enumerate() {
        if c == '.' {
            if non_zero_encountered {
                return (i as i32) - 1;
            }
            dot_encountered = true;
        } else if c != '0' {
            if dot_encountered {
                return 1 - (i as i32);
            }
            non_zero_encountered = true;
        }
    }
    (flt.len() as i32) - 1
}

/// round_to_precision - used in to_precision
///
/// This procedure has two roles:
/// - If there are enough or more than enough digits in the
///   string to show the required precision, the number
///   represented by these digits is rounded using string
///   manipulation.
/// - Else, zeroes are appended to the string.
///
/// When this procedure returns, `digits` is exactly `precision` long.
///
pub fn round_to_precision(digits: &mut String, precision: usize) -> String {
    if digits.len() > precision {
        let to_round = digits.split_off(precision);
        let mut digit_bytes = digits.clone().into_bytes();
        let mut stop_index = (digit_bytes.len() - 1) as i32;
        let mut digit = *digit_bytes.last().unwrap();

        for c in to_round.chars() {
            match c {
                c if c < '4' => break,
                c if c > '4' => {
                    while digit == b'9' {
                        digit_bytes[stop_index as usize] = b'0';
                        stop_index -= 1;
                        if stop_index == -1 {
                            break;
                        }
                        digit = digit_bytes[stop_index as usize];
                    }
                    break;
                }
                _ => {}
            }
        }
        if stop_index == -1 {
            digit_bytes.insert(0, b'1');
            digit_bytes.pop();
        } else {
            digit_bytes[stop_index as usize] += 1;
        }
        return String::from_utf8(digit_bytes).unwrap();
    } else {
        digits.push_str(&"0".repeat(precision - digits.len()));
        return digits.to_string();
    }
}

pub fn round_to_fixed(string: &mut String, fixed: usize) -> String {
    if let Some(n) = string.find('.') {
        if (string.len() - 1 - n) > fixed {
            let to_round = string.split_off(n + fixed + 1);
            let mut digit_bytes = string.clone().into_bytes();
            let mut stop_index = (digit_bytes.len() - 1) as i32;
            let mut digit = *digit_bytes.last().unwrap();

            for c in to_round.chars() {
                match c {
                    c if c < '4' => break,
                    c if c > '4' => {
                        loop {
                            if digit == b'9' {
                                digit_bytes[stop_index as usize] = b'0';
                                stop_index -= 1;
                                if stop_index == -1 {
                                    break;
                                }
                                digit = digit_bytes[stop_index as usize];
                            } else if digit == b'.' {
                                stop_index -= 1;
                                digit = digit_bytes[stop_index as usize];
                            } else {
                                break;
                            }
                        }
                        break;
                    }
                    _ => {}
                }
            }
            if stop_index == -1 {
                digit_bytes.insert(0, b'1');
            } else {
                digit_bytes[stop_index as usize] += 1;
            }
            if fixed == 0 {
                digit_bytes.pop();
            }
            return String::from_utf8(digit_bytes).unwrap();
        } else {
            let mut digits = string.split_off(n + 1);
            digits = round_to_precision(&mut digits, fixed);
            string.push_str(&digits);
            return string.to_string();
        }
    } else {
        let mut digits = String::from("");
        digits = round_to_precision(&mut digits, fixed);
        if !digits.is_empty() {
            string.push('.');
            string.push_str(&digits);
        }
        return string.to_string();
    }
}

impl GcPointer<Context> {
    pub(crate) fn init_number_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.number_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();
        let mut global_object = self.global_object();
        def_native_property!(self, global_object, Number, constructor, W | C)?;
        Ok(())
    }

    pub(crate) fn init_number_in_global_data(
        mut self,
        obj_proto: GcPointer<JsObject>,
    ) -> Result<(), JsValue> {
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = NumberObject::new_plain(self, structure, 0.0);

        self.global_data
            .number_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);

        let mut constructor = JsNativeFunction::new(self, "Number".intern(), number_constructor, 1);

        def_native_property!(self, constructor, prototype, proto, NONE)?;
        def_native_property!(self, constructor, MAX_VALUE, f64::MAX)?;
        def_native_property!(self, constructor, MIN_VALUE, f64::MIN)?;
        def_native_property!(self, constructor, NaN, f64::NAN)?;
        def_native_property!(self, constructor, NEGATIVE_INFINITY, f64::NEG_INFINITY)?;
        def_native_property!(self, constructor, POSITIVE_INFINITY, f64::INFINITY)?;
        def_native_property!(self, constructor, EPSILON, f64::EPSILON)?;
        def_native_property!(self, constructor, MAX_SAFE_INTEGER, 9007199254740991.0)?;
        def_native_method!(self, constructor, isNaN, number_is_nan, 0)?;
        def_native_method!(self, constructor, isFinite, number_is_finite, 0)?;
        def_native_method!(self, constructor, isInteger, number_is_integer, 0)?;

        def_native_property!(self, proto, constructor, constructor)?;
        def_native_method!(self, proto, toString, number_to_string, 1)?;
        def_native_method!(self, proto, valueOf, number_value_of, 0)?;
        def_native_method!(self, proto, toPrecision, number_to_precisiion, 1)?;
        def_native_method!(self, proto, toFixed, number_to_fixed, 1)?;
        def_native_method!(self, proto, clz, number_clz, 1)?;

        self.global_data.number_prototype = Some(proto);

        Ok(())
    }
}
