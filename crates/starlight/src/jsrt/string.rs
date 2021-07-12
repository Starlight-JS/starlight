use regress::Regex;

use crate::{gc::cell::GcPointer, vm::{Context, arguments::Arguments, array::JsArray, attributes::*, error::{JsRangeError, JsTypeError}, function::JsNativeFunction, object::JsObject, property_descriptor::DataDescriptor, string::{JsString, JsStringObject}, structure::Structure, symbol_table::{Internable, Symbol}, value::*}};
use std::{
    char::{decode_utf16, from_u32},
    cmp::{max, min},
    intrinsics::unlikely,
};

use super::regexp::RegExp;

pub fn string_to_string(_ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn string_concat(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let val = args.this;
    val.check_object_coercible(ctx)?;
    let mut str = val.to_string(ctx)?;
    for i in 0..args.size() {
        let arg = args.at(i);
        let r = arg.to_string(ctx)?;
        str.push_str(&r);
    }
    Ok(JsValue::encode_object_value(JsString::new(ctx, str)))
}

pub fn string_value_of(_ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn string_char_at(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let pos = args.at(0).to_int32(ctx)?;
    if pos < 0 || pos >= primitive_val.len() as i32 {
        return Ok(JsValue::encode_undefined_value());
    }

    if let Some(utf16_val) = primitive_val.encode_utf16().nth(pos as usize) {
        Ok(JsValue::new(JsString::new(
            ctx,
            from_u32(utf16_val as u32).unwrap().to_string(),
        )))
    } else {
        Ok(JsValue::new(JsString::new(ctx, "")))
    }
}

pub fn string_code_point_at(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let pos = args.at(0).to_int32(ctx)?;
    if pos < 0 || pos >= primitive_val.len() as i32 {
        return Ok(JsValue::encode_undefined_value());
    }
    if let Some((code_point, _, _)) = code_point_at(&primitive_val, pos as _) {
        Ok(JsValue::new(code_point))
    } else {
        Ok(JsValue::encode_undefined_value())
    }
}

pub fn string_char_code_at(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let pos = args.at(0).to_int32(ctx)?;
    if pos < 0 || pos >= primitive_val.len() as i32 {
        return Ok(JsValue::encode_nan_value());
    }

    if let Some(utf16_val) = primitive_val.encode_utf16().nth(pos as _) {
        return Ok(JsValue::new(utf16_val));
    } else {
        return Ok(JsValue::encode_nan_value());
    }
}

pub fn string_replace(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    if args.size() == 0 {
        return Ok(JsValue::new(JsString::new(ctx, primitive_val)));
    }

    let (regex_body, flags) = get_regex_string(ctx, args.at(0))?;
    let re = Regex::with_flags(&regex_body, flags.as_str())
        .expect("unable to convectx regex to regex object");

    let mat = match re.find(&primitive_val) {
        Some(mat) => mat,
        None => return Ok(JsValue::new(JsString::new(ctx, primitive_val))),
    };
    let caps = re
        .find(&primitive_val)
        .expect("unable to get capture groups from text")
        .captures;

    let replace_value = if args.size() > 1 {
        let val = args.at(1).to_string(ctx)?;
        let mut result = String::new();
        let mut chars = val.chars().peekable();

        let m = caps.len();

        while let Some(first) = chars.next() {
            if first == '$' {
                let second = chars.next();
                let second_is_digit = second.map_or(false, |ch| ch.is_digit(10));
                // we use peek so that it is still in the iterator if not used
                let third = if second_is_digit { chars.peek() } else { None };
                let third_is_digit = third.map_or(false, |ch| ch.is_digit(10));

                match (second, third) {
                    (Some('$'), _) => {
                        // $$
                        result.push('$');
                    }
                    (Some('&'), _) => {
                        // $&
                        result.push_str(&primitive_val[mat.range()]);
                    }
                    (Some('`'), _) => {
                        // $`
                        let start_of_match = mat.start();
                        result.push_str(&primitive_val[..start_of_match]);
                    }
                    (Some('\''), _) => {
                        // $'
                        let end_of_match = mat.end();
                        result.push_str(&primitive_val[end_of_match..]);
                    }
                    (Some(second), Some(third)) if second_is_digit && third_is_digit => {
                        // $nn
                        let tens = second.to_digit(10).unwrap() as usize;
                        let units = third.to_digit(10).unwrap() as usize;
                        let nn = 10 * tens + units;
                        if nn == 0 || nn > m {
                            result.push(first);
                            result.push(second);
                            if let Some(ch) = chars.next() {
                                result.push(ch);
                            }
                        } else {
                            let group = match mat.group(nn) {
                                Some(range) => &primitive_val[range.clone()],
                                _ => "",
                            };
                            result.push_str(group);
                            chars.next(); // consume third
                        }
                    }
                    (Some(second), _) if second_is_digit => {
                        // $n
                        let n = second.to_digit(10).unwrap() as usize;
                        if n == 0 || n > m {
                            result.push(first);
                            result.push(second);
                        } else {
                            let group = match mat.group(n) {
                                Some(range) => &primitive_val[range.clone()],
                                _ => "",
                            };
                            result.push_str(group);
                        }
                    }
                    (Some('<'), _) => {
                        // $<
                        // TODO: named capture groups
                        result.push_str("$<");
                    }
                    _ => {
                        // $?, ? is none of the above
                        // we can consume second because it isn't $
                        result.push(first);
                        if let Some(second) = second {
                            result.push(second);
                        }
                    }
                }
            } else {
                result.push(first);
            }
        }

        result
    } else {
        "undefined".to_string()
    };
    Ok(JsValue::new(JsString::new(
        ctx,
        primitive_val.replace(&primitive_val[mat.range()], &replace_value),
    )))
}

pub fn string_index_of(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    args.this.check_object_coercible(ctx)?;
    let string = args.this.to_string(ctx)?;
    let search_string = args.at(0).to_string(ctx)?;

    let length = string.chars().count();

    let start = if args.size() > 1 {
        args.at(1).to_int32(ctx)?
    } else {
        0
    };
    let start = (start.max(0).min(length as i32)) as usize;

    if search_string.is_empty() {
        return Ok(JsValue::new(start as u32));
    }

    if start < length {
        if let Some(pos) = string[start..].find(search_string.as_str()) {
            return Ok(JsValue::new(string[..pos].chars().count() as u32));
        }
    }

    Ok(JsValue::new(-1))
}

pub fn string_last_index_of(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    args.this.check_object_coercible(ctx)?;
    let string: String = args.this.to_string(ctx)?;
    let search_string: String = args.at(0).to_string(ctx)?;

    let length = string.chars().count();
    let search_string_length = search_string.chars().count();
    let max_search_index = (length - search_string_length) as i32;
    let start = if args.size() > 1 {
        args.at(1).to_int32(ctx)?
    } else {
        max_search_index
    };

    let start = (start.max(0).min(max_search_index)) as usize;
    if search_string.is_empty() {
        return Ok(JsValue::new(start as u32));
    }

    if let Some(pos) = string[..(start + search_string_length)].rfind(search_string.as_str()) {
        return Ok(JsValue::new(string[..pos].chars().count() as u32));
    }

    Ok(JsValue::new(-1))
}

pub fn string_repeat(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    args.this.check_object_coercible(ctx)?;
    let object = args.this.to_string(ctx)?;
    if args.size() > 0 {
        let n = args.at(0).to_int32(ctx)?;
        if unlikely(n < 0) {
            let msg = JsString::new(ctx, "repeat count cannot be a negative number");
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }

        if unlikely(n as usize * object.len() >= u32::MAX as usize - 1) {
            let msg = JsString::new(ctx, "repeat count must not overflow max string length");
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }
        Ok(JsValue::new(JsString::new(ctx, object.repeat(n as _))))
    } else {
        Ok(JsValue::new(JsString::new(ctx, "")))
    }
}
pub fn string_to_lowercase(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_string(ctx)?;
    Ok(JsValue::new(JsString::new(ctx, this.to_lowercase())))
}

pub fn string_to_uppercase(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_string(ctx)?;
    Ok(JsValue::new(JsString::new(ctx, this.to_uppercase())))
}
pub fn string_starts_with(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let arg = args.at(0);
    if unlikely(arg.is_jsobject() && arg.get_jsobject().is_class(RegExp::get_class())) {
        let msg = JsString::new(
            ctx,
            "First argument to String.prototype.endsWith must not be a regular expression",
        );
        return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
    }
    let search_string = arg.to_string(ctx)?;
    let length = primitive_val.chars().count() as i32;
    let search_length = search_string.chars().count() as i32;
    let position = if args.size() < 2 {
        0
    } else {
        args.at(1).to_int32(ctx)?
    };

    let start = min(max(position, 0), length);
    let end = start.wrapping_add(search_length);
    if end > length {
        Ok(JsValue::new(false))
    } else {
        let this_string = primitive_val
            .chars()
            .skip(start as usize)
            .collect::<String>();
        Ok(JsValue::new(
            this_string.starts_with(search_string.as_str()),
        ))
    }
}

pub fn string_ends_with(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let arg = args.at(0);
    if unlikely(arg.is_jsobject() && arg.get_jsobject().is_class(RegExp::get_class())) {
        let msg = JsString::new(
            ctx,
            "First argument to String.prototype.startsWith must not be a regular expression",
        );
        return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
    }
    let search_string = arg.to_string(ctx)?;
    let length = primitive_val.chars().count() as i32;
    let search_length = search_string.chars().count() as i32;
    let position = if args.size() < 2 {
        0
    } else {
        args.at(1).to_int32(ctx)?
    };

    let start = min(max(position, 0), length);
    let end = start.wrapping_add(search_length);
    if end > length {
        Ok(JsValue::new(false))
    } else {
        let this_string = primitive_val
            .chars()
            .skip(start as usize)
            .collect::<String>();
        Ok(JsValue::new(
            this_string.starts_with(search_string.as_str()),
        ))
    }
}

pub fn string_includes(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let arg = args.at(0);
    if unlikely(arg.is_jsobject() && arg.get_jsobject().is_class(RegExp::get_class())) {
        let msg = JsString::new(
            ctx,
            "First argument to String.prototype.startsWith must not be a regular expression",
        );
        return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
    }
    let search_string = arg.to_string(ctx)?;
    let length = primitive_val.chars().count() as i32;

    let position = if args.size() < 2 {
        0
    } else {
        args.at(1).to_int32(ctx)?
    };

    let start = min(max(position, 0), length);
    let this_string = primitive_val.chars().skip(start as _).collect::<String>();
    Ok(JsValue::new(this_string.contains(search_string.as_str())))
}
pub fn string_slice(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let length = primitive_val.chars().count() as i32;
    let start = args.at(0).to_int32(ctx)?;
    let end = if args.size() > 1 {
        args.at(1).to_int32(ctx)?
    } else {
        length as i32
    };
    let from = if start < 0 {
        max(length.wrapping_add(start as i32), 0)
    } else {
        min(start, length as i32)
    };
    let to = if end < 0 {
        max(length.wrapping_add(end as _), 0)
    } else {
        min(end, length as i32)
    };

    let span = max(to.wrapping_sub(from), 0);

    let new_str = primitive_val
        .chars()
        .skip(from as usize)
        .take(span as usize)
        .collect::<String>();
    Ok(JsValue::new(JsString::new(ctx, new_str)))
}
pub fn string_substring(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let start = if args.size() == 0 {
        0
    } else {
        args.at(0).to_int32(ctx)?
    };

    let length = primitive_val.encode_utf16().count() as i32;
    let end = if args.size() < 2 {
        length
    } else {
        args.at(1).to_int32(ctx)?
    };

    let final_start = min(max(start, 0), length);
    let final_end = min(max(end, 0), length);
    let from = min(final_start, final_end) as usize;
    let to = max(final_start, final_end) as usize;

    let extracted_string: Result<String, _> = decode_utf16(
        primitive_val
            .encode_utf16()
            .skip(from)
            .take(to.wrapping_sub(from)),
    )
    .collect();

    match extracted_string {
        Ok(val) => return Ok(JsValue::new(JsString::new(ctx, val))),
        Err(e) => {
            let decode_err = JsString::new(ctx, format!("{}", e));
            Err(JsValue::new(decode_err))
        }
    }
}

pub fn string_substr(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let primitive_val = args.this.to_string(ctx)?;
    let mut start = if args.size() == 0 {
        0
    } else {
        args.at(0).to_int32(ctx)?
    };

    let length = primitive_val.chars().count() as i32;

    let end = if args.size() < 2 {
        i32::MAX
    } else {
        args.at(1).to_int32(ctx)?
    };

    if start < 0 {
        start = max(length.wrapping_add(start), 0);
    }

    let result_length = min(max(end, 0), length.wrapping_sub(start));

    if result_length <= 0 {
        return Ok(JsValue::new(JsString::new(ctx, "")));
    }

    let extracted_string: String = primitive_val
        .chars()
        .skip(start as _)
        .take(result_length as _)
        .collect();
    Ok(JsValue::new(JsString::new(ctx, extracted_string)))
}

pub fn string_split(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let val = args.this;
    val.check_object_coercible(ctx)?;
    let str = val.to_string(ctx)?;

    let argc = args.size();
    let lim = if argc < 2 || args.at(1).is_undefined() {
        4294967295u32
    } else {
        args.at(1).to_uint32(ctx)?
    };
    let separator = if args.at(0).is_undefined() || args.at(0).is_null() {
        None
    } else {
        Some(args.at(0).to_string(ctx)?)
    };
    let values = match separator {
        None if lim == 0 => vec![],
        None => vec![JsValue::encode_object_value(JsString::new(ctx, str))],
        Some(separator) if separator.is_empty() => str
            .encode_utf16()
            .map(|cp| {
                JsValue::encode_object_value(JsString::new(ctx, String::from_utf16_lossy(&[cp])))
            })
            .take(lim as _)
            .collect(),
        Some(separator) => str
            .split(separator.as_str())
            .map(|x| JsValue::encode_object_value(JsString::new(ctx, x)))
            .take(lim as _)
            .collect(),
    };

    let mut arr = JsArray::new(ctx, values.len() as _);
    for (ix, item) in values.iter().enumerate() {
        arr.put(ctx, Symbol::Index(ix as _), *item, false)?;
    }
    Ok(JsValue::encode_object_value(arr))
}

pub fn string_constructor(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.ctor_call {
        let str;
        if args.size() != 0 {
            str = args.at(0).to_string(ctx)?;
        } else {
            str = "".to_owned();
        }
        let _ = str;
        let msg = JsString::new(
            ctx,
            "String.prototype.constructor as constructor is not yet implemented",
        );
        Ok(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )))
    } else if args.size() != 0 {
        let str = args.at(0).to_string(ctx)?;
        let jsttr = JsString::new(ctx, str);
        return Ok(JsValue::encode_object_value(jsttr));
    } else {
        let jsttr = JsString::new(ctx, "");
        return Ok(JsValue::encode_object_value(jsttr));
    }
}

impl Context {

    pub fn init_string_in_realm(&mut self){
        let name = "constructor".intern();
        let constructor = self.global_data.string_prototype.unwrap().get_own_property(self, name).unwrap().value();
        self.global_object()
            .put(
                self,
                "String".intern(),
                constructor,
                false,
            )
            .unwrap_or_else(|_| panic!());
    }

    pub fn init_string_in_global_data(&mut self,obj_proto: GcPointer<JsObject>) {
        self.global_data.string_structure = Some(Structure::new_indexed(self, None, true));
        let map = Structure::new_unique_with_proto(self, Some(obj_proto), false);
        let mut proto = JsStringObject::new_plain(self, &map);

        self.global_data
            .string_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);
        let mut ctor = JsNativeFunction::new(self, "String".intern(), string_constructor, 1);

        let _ = ctor.define_own_property(
            self,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::new(proto), NONE),
            false,
        );

        proto
            .define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::new(ctor), W | C),
                false,
            )
            .unwrap_or_else(|_| panic!());
        let func = JsNativeFunction::new(self, "toString".intern(), string_to_string, 0);
        proto
            .put(
                self,
                "toString".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| panic!());
        let func = JsNativeFunction::new(self, "valueOf".intern(), string_value_of, 0);
        proto
            .put(
                self,
                "valueOf".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| panic!());

        let func = JsNativeFunction::new(self, "___splitFast".intern(), string_split, 0);
        proto
            .put(
                self,
                "___splitFast".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| panic!());

        let func = JsNativeFunction::new(self, "concat".intern(), string_concat, 0);
        proto
            .put(
                self,
                "concat".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| panic!());

        let mut init = || -> Result<(), JsValue> {
            def_native_method!(self, proto, charAt, string_char_at, 1)?;
            def_native_method!(self, proto, charCodeAt, string_char_code_at, 1)?;
            def_native_method!(self, proto, toUpperCase, string_to_uppercase, 0)?;
            def_native_method!(self, proto, toLowerCase, string_to_lowercase, 0)?;
            def_native_method!(self, proto, indexOf, string_index_of, 2)?;
            def_native_method!(self, proto, lastIndexOf, string_last_index_of, 2)?;
            def_native_method!(self, proto, substr, string_substr, 2)?;
            def_native_method!(self, proto, substring, string_substring, 2)?;
            def_native_method!(self, proto, codePointAt, string_code_point_at, 1)?;
            def_native_method!(self, proto, repeat, string_repeat, 1)?;
            def_native_method!(self, proto, startsWith, string_starts_with, 1)?;
            def_native_method!(self, proto, endsWith, string_ends_with, 1)?;
            def_native_method!(self, proto, includes, string_includes, 1)?;
            def_native_method!(self, proto, slice, string_slice, 1)?;
            def_native_method!(self, ctor, ___replace, string_replace, 2)?;
            def_native_method!(self, proto, trim, string_trim, 0)?;
            def_native_method!(self, proto, trimStactx, string_trim_start, 0)?;
            def_native_method!(self, proto, trimEnd, string_trim_end, 0)?;
            def_native_method!(self, proto, trimLeft, string_trim_start, 0)?;
            def_native_method!(self, proto, trimRight, string_trim_end, 0)?;
            def_native_method!(self, proto, padStactx, string_pad_start, 2)?;
            def_native_method!(self, proto, padEnd, string_pad_end, 2)?;
            def_native_method!(self, proto, repeat, string_repeat, 1)?;
            Ok(())
        };

        match init() {
            Ok(_) => (),
            _ => unreachable!(),
        }
        self.global_data.string_prototype = Some(proto);
    }
}

pub(crate) fn code_point_at(string: &str, position: i32) -> Option<(u32, u8, bool)> {
    let size = string.encode_utf16().count() as i32;
    if position < 0 || position >= size {
        return None;
    }
    let mut encoded = string.encode_utf16();
    let first = encoded.nth(position as usize)?;
    if !is_leading_surrogate(first) && !is_trailing_surrogate(first) {
        return Some((first as u32, 1, false));
    }
    if is_trailing_surrogate(first) || position + 1 == size {
        return Some((first as u32, 1, true));
    }
    let second = encoded.next()?;
    if !is_trailing_surrogate(second) {
        return Some((first as u32, 1, true));
    }
    let cp = (first as u32 - 0xD800) * 0x400 + (second as u32 - 0xDC00) + 0x10000;
    Some((cp, 2, false))
}

/// Helper function to check if a `char` is trimmable.
#[inline]
#[allow(dead_code)]
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

fn is_leading_surrogate(value: u16) -> bool {
    (0xD800..=0xDBFF).contains(&value)
}

fn is_trailing_surrogate(value: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&value)
}

fn get_regex_string(_ctx: &mut Context, val: JsValue) -> Result<(String, String), JsValue> {
    if val.is_jsstring() {
        return Ok((val.get_jsstring().string.clone(), String::new()));
    }
    if val.is_jsobject() {
        let obj = val.get_jsobject();
        if obj.is_class(RegExp::get_class()) {
            return Ok((
                obj.data::<RegExp>().original_source.to_string(),
                obj.data::<RegExp>().original_flags.to_string(),
            ));
        }
    }
    return Ok(("undefined".to_string(), "".to_string()));
}

pub fn string_trim(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let prim = args.this.to_string(ctx)?;
    Ok(JsValue::new(JsString::new(ctx, prim.trim())))
}

pub fn string_trim_start(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let prim = args.this.to_string(ctx)?;
    Ok(JsValue::new(JsString::new(ctx, prim.trim_start())))
}

pub fn string_trim_end(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    let prim = args.this.to_string(ctx)?;
    Ok(JsValue::new(JsString::new(ctx, prim.trim_end())))
}

pub enum Alignment {
    Stactx,
    End,
}

pub fn string_pad(
    ctx: &mut Context,
    args: &Arguments,
    alignment: Alignment,
) -> Result<JsValue, JsValue> {
    let mut string = args.this.to_string(ctx)?;
    let target_length = args.at(0).to_int32(ctx)?;
    let pad_str_arg = args.at(1);
    let mut pad_str = String::from(" ");
    if !pad_str_arg.is_undefined() {
        pad_str = pad_str_arg.to_string(ctx)?;
    }
    let length = string.chars().count();
    if target_length <= length as i32 || pad_str.is_empty() {
        Ok(JsValue::new(JsString::new(ctx, string)))
    } else {
        let pad_num = target_length as usize - length;

        let mut pad_str_iter = pad_str.chars();
        let mut to_pad_str = String::from("");
        let mut index = 0;
        while index < pad_num {
            match pad_str_iter.next() {
                Some(ch) => {
                    to_pad_str.push(ch);
                    index += 1;
                }
                None => {
                    pad_str_iter = pad_str.chars();
                }
            }
        }
        match alignment {
            Alignment::Stactx => {
                to_pad_str.push_str(&string);
                Ok(JsValue::new(JsString::new(ctx, to_pad_str)))
            }
            Alignment::End => {
                string.push_str(&to_pad_str);
                Ok(JsValue::new(JsString::new(ctx, string)))
            }
        }
    }
}

pub fn string_pad_end(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    string_pad(ctx, args, Alignment::End)
}

pub fn string_pad_start(ctx: &mut Context, args: &Arguments) -> Result<JsValue, JsValue> {
    string_pad(ctx, args, Alignment::Stactx)
}
