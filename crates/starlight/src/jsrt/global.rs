/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{prelude::JsString, vm::{context::Context, arguments::Arguments, value::*}};
use num::traits::*;
use std::io::Write;
pub fn parse_float(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if !args.size() != 0 {
        let str = args.at(0).to_string(ctx)?;

        Ok(JsValue::encode_untrusted_f64_value(
            str.parse::<f64>().unwrap_or(std::f64::NAN),
        ))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}
/// This function is taken from Boa.
/// Helper function to check if a `char` is trimmable.
#[inline]
pub(crate) fn is_trimmable_whitespace(c: char) -> bool {
    // The rust implementation of `trim` does not regard the same characters whitespace as ecma standard does
    //
    // Rust uses \p{White_Space} by default, which also includes:
    // `\u{0085}' (next line)
    // And does not include:
    // '\u{FEFF}' (zero width non-breaking space)
    // Explicit whitespace: https://tc39.es/ecma262/#sec-white-space
    matches!(
        c,
        '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{0020}' | '\u{00A0}' | '\u{FEFF}' |
    // Unicode Space_Separator category
    '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' |
    // Line terminators: https://tc39.es/ecma262/#sec-line-terminators
    '\u{000A}' | '\u{000D}' | '\u{2028}' | '\u{2029}'
    )
}

pub fn parse_int(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() >= 1 {
        let str = args.at(0).to_string(ctx)?;
        let mut var_s = str.trim_start_matches(is_trimmable_whitespace);

        let sign = if !var_s.is_empty() && var_s.starts_with('\u{002D}') {
            -1
        } else {
            1
        };

        if !var_s.is_empty() {
            var_s = var_s
                .strip_prefix(&['\u{002B}', '\u{002D}'][..])
                .unwrap_or(var_s);
        }
        let mut var_r = args.at(1).to_int32(ctx)?;
        // 7. Let stripPrefix be true.
        let mut strip_prefix = true;

        // 8. If R ≠ 0, then
        if var_r != 0 {
            //     a. If R < 2 or R > 36, return NaN.
            if !(2..=36).contains(&var_r) {
                return Ok(JsValue::encode_nan_value());
            }

            //     b. If R ≠ 16, set stripPrefix to false.
            if var_r != 16 {
                strip_prefix = false
            }
        } else {
            // 9. Else,
            //     a. Set R to 10.
            var_r = 10;
        }

        // 10. If stripPrefix is true, then
        //     a. If the length of S is at least 2 and the first two code units of S are either "0x" or "0X", then
        //         i. Remove the first two code units from S.
        //         ii. Set R to 16.
        if strip_prefix && var_s.len() >= 2 && (var_s.starts_with("0x") || var_s.starts_with("0X"))
        {
            var_s = var_s.split_at(2).1;

            var_r = 16;
        }

        // 11. If S contains a code unit that is not a radix-R digit, let end be the index within S of the
        //     first such code unit; otherwise, let end be the length of S.
        let end = if let Some(index) = var_s.find(|c: char| !c.is_digit(var_r as u32)) {
            index
        } else {
            var_s.len()
        };

        // 12. Let Z be the substring of S from 0 to end.
        let var_z = var_s.split_at(end).0;

        // 13. If Z is empty, return NaN.
        if var_z.is_empty() {
            return Ok(JsValue::encode_nan_value());
        }

        // 14. Let mathInt be the integer value that is represented by Z in radix-R notation, using the
        //     letters A-Z and a-z for digits with values 10 through 35. (However, if R is 10 and Z contains
        //     more than 20 significant digits, every significant digit after the 20th may be replaced by a
        //     0 digit, at the option of the implementation; and if R is not 2, 4, 8, 10, 16, or 32, then
        //     mathInt may be an implementation-approximated value representing the integer value that is
        //     represented by Z in radix-R notation.)
        let math_int = u64::from_str_radix(var_z, var_r as u32).map_or_else(
            |_| f64::from_str_radix(var_z, var_r as u32).expect("invalid_float_conversion"),
            |i| i as f64,
        );

        // 15. If mathInt = 0, then
        //     a. If sign = -1, return -0𝔽.
        //     b. Return +0𝔽.
        if math_int == 0_f64 {
            if sign == -1 {
                return Ok(JsValue::new(-0_f64));
            } else {
                return Ok(JsValue::new(0_f64));
            }
        }

        // 16. Return 𝔽(sign × mathInt).
        Ok(JsValue::new(sign as f64 * math_int))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn is_nan(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let val = args.at(0);
        let number = val.to_number(ctx)?;
        return Ok(JsValue::encode_bool_value(number.is_nan()));
    }
    Ok(JsValue::encode_bool_value(true))
}

pub fn is_finite(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let val = args.at(0);
        let number = val.to_number(ctx)?;
        return Ok(JsValue::encode_bool_value(number.is_finite()));
    }
    Ok(JsValue::encode_bool_value(false))
}

pub fn gc(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    ctx.heap().gc();
    let _ = args;
    Ok(JsValue::encode_undefined_value())
}

pub fn ___trunc(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let n = args.at(0).to_number(ctx)?.trunc();
    Ok(JsValue::new(n))
}

pub fn ___is_callable(_: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(args.at(0).is_callable()))
}

pub fn to_string(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    args.at(0)
        .to_string(ctx)
        .map(|x| JsValue::new(JsString::new(ctx, x)))
}

// TODO: Breakpoints
pub fn __breakpoint(_ctx: &mut Context, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!()
}
pub fn __breakpoint_noop(_ctx: &mut Context, _args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::encode_undefined_value())
}

pub fn ___is_constructor(_ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg = args.at(0);
    if arg.is_callable() {
        let fun = arg.get_jsobject();
        return Ok(JsValue::new(
            fun.as_function().is_native()
                || (fun.as_function().is_vm() && fun.as_function().as_vm().code.is_constructor),
        ));
    }

    Ok(JsValue::new(false))
}

pub fn read_line(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let prompt = if args.size() > 0 {
        Some(args.at(0).to_string(ctx)?)
    } else {
        None
    };

    if let Some(prompt) = prompt {
        print!("{}", prompt);
        std::io::stdout().flush().unwrap();
    }

    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();

    Ok(JsValue::new(JsString::new(ctx, buf)))
}
