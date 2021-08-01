/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::{intrinsics::unlikely, u32};

use super::object::object_to_string;
use crate::{
    constant::S_CONSTURCTOR,
    gc::cell::GcPointer,
    jsrt::{array, get_length},
    vm::{
        arguments::*, array::*, attributes::*, builder::Builtin, class::JsClass, context::Context,
        error::*, function::JsNativeFunction, object::*, property_descriptor::DataDescriptor,
        string::*, structure::Structure, symbol_table::*, value::*,
    },
};
pub fn array_ctor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let size = args.size();
    if size == 0 {
        return Ok(JsValue::encode_object_value(JsArray::new(ctx, 0)));
    }
    if size == 1 {
        let first = args.at(0);
        if first.is_number() {
            let val = first.to_number(ctx)?;
            let len = val as u32;
            if len as f64 == val {
                return Ok(JsValue::encode_object_value(JsArray::new(ctx, len)));
            } else {
                let msg = JsString::new(ctx, format!("invalid array length '{}", len));

                return Err(JsValue::encode_object_value(JsRangeError::new(
                    ctx, msg, None,
                )));
            }
        } else {
            let mut ary = JsArray::new(ctx, 1);
            ary.put(ctx, Symbol::Index(0), first, false)?;

            Ok(JsValue::encode_object_value(ary))
        }
    } else {
        let mut ary = JsArray::new(ctx, size as _);
        for i in 0..size {
            ary.define_own_property(
                ctx,
                Symbol::Index(i as _),
                &*DataDescriptor::new(args.at(i), W | C | E),
                false,
            )?;
        }
        Ok(JsValue::encode_object_value(ary))
    }
}

pub fn array_is_array(_ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::encode_bool_value(false));
    }
    let val = args.at(0);
    if !val.is_jsobject() {
        return Ok(JsValue::encode_bool_value(false));
    }
    Ok(JsValue::encode_bool_value(
        val.get_object().downcast::<JsObject>().unwrap().tag() == ObjectTag::Array,
    ))
}

pub fn array_of(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut ary = JsArray::new(ctx, args.size() as _);
    for i in 0..args.size() {
        ary.put(ctx, Symbol::Index(i as _), args.at(i), false)?;
    }
    Ok(JsValue::encode_object_value(ary))
}

pub fn array_from(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(arg1 = stack, args.at(0).to_object(ctx)?);
    let len = arg1.get(ctx, "length".intern())?;
    let len = if len.is_number() {
        let n = len.to_number(ctx)?;
        if n as u32 as f64 == n {
            n as u32
        } else {
            0
        }
    } else {
        0
    };
    let mut target = JsArray::new(ctx, len);
    for k in 0..len {
        if arg1.has_property(ctx, Symbol::Index(k)) {
            let value = arg1.get(ctx, Symbol::Index(k))?;
            target.put(ctx, Symbol::Index(k), value, false)?;
        }
    }

    Ok(JsValue::encode_object_value(target))
}
pub fn array_join(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(obj = stack, args.this.to_object(ctx)?);
    let len = obj.get(ctx, "length".intern())?.to_number(ctx)?;
    let len = if len as u32 as f64 == len {
        len as u32
    } else {
        0_u32
    };
    let separator = if !args.at(0).is_undefined() {
        args.at(0).to_string(ctx)?
    } else {
        ",".to_string()
    };

    let mut fmt = String::new();
    {
        let element0 = obj.get(ctx, Symbol::Index(0))?;
        if !(element0.is_undefined() || element0.is_null()) {
            let str = element0.to_string(ctx)?;
            fmt.push_str(&str);
        }
    }

    let mut k: u32 = 1;
    while k < len {
        fmt.push_str(&separator);
        let element = obj.get(ctx, Symbol::Index(k))?;
        if !(element.is_undefined() || element.is_null()) {
            let str = element.to_string(ctx)?;
            fmt.push_str(&str);
        }
        k += 1;
    }
    Ok(JsValue::encode_object_value(JsString::new(ctx, fmt)))
}
pub fn array_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(this = stack, args.this.to_object(ctx)?);
    let m = this.get_property(ctx, "join".intern());
    if m.value().is_callable() {
        letroot!(func = stack, unsafe {
            m.value().get_object().downcast_unchecked::<JsObject>()
        });
        letroot!(f2 = stack, *func);
        let f = func.as_function_mut();
        letroot!(args = stack, Arguments::new(args.this, &mut []));
        return f.call(ctx, &mut args, JsValue::new(*f2));
    }
    letroot!(args = stack, Arguments::new(args.this, &mut []));
    object_to_string(ctx, &args)
}

// TODO(playX): Allow to push up to 2^53-1 values
pub fn array_push(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(ctx)?;
    let n = obj.get(ctx, "length".intern())?.to_number(ctx)?;
    let mut n = if n as u32 as f64 == n {
        n as u32 as u64
    } else {
        let msg = JsString::new(ctx, "invalid length");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            ctx, msg, None,
        )));
    };
    // let p = n;
    let max = 0x100000000u64;
    let mut it = 0;
    let last = args.size();
    if (n + args.size() as u64) <= max {
        while it != last {
            obj.put(ctx, Symbol::Index(n as _), args.at(it), false)?;
            it += 1;
            n += 1;
        }
    } else {
        let msg = JsString::new(ctx, "array size exceeded");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            ctx, msg, None,
        )));
    }
    let len = n as f64;
    obj.put(ctx, "length".intern(), JsValue::new(len), false)?;
    Ok(JsValue::new(n as f64))
}

pub fn array_pop(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(ctx)?;
    let n = obj.get(ctx, "length".intern())?.to_number(ctx)?;
    let len = if n as u32 as f64 == n {
        n as u32
    } else {
        let msg = JsString::new(ctx, "invalid length");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            ctx, msg, None,
        )));
    };
    if len == 0 {
        obj.put(ctx, "length".intern(), JsValue::new(0.0), true)?;
        return Ok(JsValue::encode_undefined_value());
    } else {
        let index = len - 1;
        let element = obj.get(ctx, Symbol::Index(index))?;
        obj.delete(ctx, Symbol::Index(index), true)?;
        obj.put(ctx, "length".intern(), JsValue::new(index as i32), true)?;
        Ok(element)
    }
}

pub fn array_reduce(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(obj = stack, args.this.to_object(ctx)?);
    let len = get_length(ctx, &mut obj)?;
    let arg_count = args.size();
    if arg_count == 0 || !args.at(0).is_callable() {
        let msg = JsString::new(
            ctx,
            "Array.prototype.reduce requires callable object as 1st argument",
        );
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }

    letroot!(callbackf = stack, args.at(0).get_jsobject());
    letroot!(cb = stack, *callbackf);
    let callback = callbackf.as_function_mut();
    if len == 0 && arg_count <= 1 {
        let msg = JsString::new(
            ctx,
            "Array.prototype.reduce with empty array requires initial value as 2nd argumentt",
        );
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }
    let mut k = 0;
    letroot!(acc = stack, JsValue::encode_undefined_value());
    if arg_count > 1 {
        *acc = args.at(1);
    } else {
        let mut k_present = false;
        while k < len {
            if obj.has_property(ctx, Symbol::Index(k)) {
                k_present = true;
                *acc = obj.get(ctx, Symbol::Index(k))?;
                k += 1;
                break;
            }
            k += 1;
        }

        if !k_present {
            let msg = JsString::new(
                ctx,
                "Array.prototype.reduce with empty array requires initial value",
            );
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }
    }

    while k < len {
        if obj.has_property(ctx, Symbol::Index(k)) {
            let mut tmp = [JsValue::encode_undefined_value(); 4];
            letroot!(
                args = stack,
                Arguments::new(JsValue::encode_undefined_value(), &mut tmp)
            );
            *args.at_mut(0) = *acc;
            *args.at_mut(1) = obj.get(ctx, Symbol::Index(k))?;
            *args.at_mut(2) = JsValue::new(k as i32);
            *args.at_mut(3) = JsValue::encode_object_value(*obj);
            *acc = callback.call(ctx, &mut args, JsValue::new(*cb))?;
        }
        k += 1;
    }
    Ok(*acc)
}

pub fn array_concat(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(args.this);
    }

    let mut ix = 0;
    if !args.this.is_jsobject() {
        let msg = JsString::new(ctx, "Array.prototype.concat requires array-like object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )));
    }
    let stack = ctx.shadowstack();
    letroot!(this = stack, args.this.get_jsobject());
    let this_length = super::get_length(ctx, &mut this)?;

    let mut new_values = JsArray::new(ctx, this_length);
    for n in 0..this_length {
        let val = this.get(ctx, Symbol::Index(n))?;
        new_values.put(ctx, Symbol::Index(ix), val, false)?;
        ix += 1;
    }

    for ai in 0..args.size() {
        let arg = args.at(ai);
        if !arg.is_jsobject() {
            let msg = JsString::new(ctx, "Array.prototype.concat requires array-like arguments");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                ctx, msg, None,
            )));
        }
        letroot!(arg = stack, arg.get_jsobject());
        let len = super::get_length(ctx, &mut arg)?;
        if unlikely(len >= u32::MAX - 1) {
            return Err(JsValue::new(ctx.new_type_error(
                "Array-like object length exceeds array length limit in Array.prototype.concat",
            )));
        }
        for n in 0..len {
            let val = arg.get(ctx, Symbol::Index(n))?;
            new_values.put(ctx, Symbol::Index(ix), val, false)?;
            ix += 1;
        }
    }

    Ok(JsValue::encode_object_value(new_values))
}

pub fn array_for_each(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(array = stack, args.this.to_object(ctx)?);
    let length = super::get_length(ctx, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(ctx.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    letroot!(callback = stack, callback.to_object(ctx)?);
    letroot!(cb2 = stack, *callback);
    let this_arg = args.at(1);
    let mut buf: [JsValue; 3] = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if array.has_property(ctx, Symbol::Index(i)) {
            let element = array.get(ctx, Symbol::Index(i))?;
            buf[0] = element;
            buf[1] = JsValue::new(i);
            buf[2] = JsValue::new(*array);
            letroot!(args = stack, Arguments::new(this_arg, &mut buf));

            callback
                .as_function_mut()
                .call(ctx, &mut args, JsValue::new(*cb2))?;
        }
    }
    Ok(JsValue::encode_undefined_value())
}

pub fn array_filter(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(array = stack, args.this.to_object(ctx)?);
    let length = super::get_length(ctx, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(ctx.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    letroot!(callback = stack, callback.to_object(ctx)?);
    letroot!(cb2 = stack, *callback);
    letroot!(result = stack, JsArray::new(ctx, 0));
    letroot!(this_arg = stack, args.at(1));

    let mut next_index = 0;
    let mut buf = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if !array.has_own_property(ctx, Symbol::Index(i)) {
            continue;
        }
        let current = array.get(ctx, Symbol::Index(i))?;
        buf[0] = current;
        buf[1] = JsValue::new(i);
        buf[2] = JsValue::new(*array);
        let mut args = Arguments::new(*this_arg, &mut buf);
        let val = callback
            .as_function_mut()
            .call(ctx, &mut args, JsValue::new(*cb2))?;
        if val.to_boolean() {
            result.put(ctx, Symbol::Index(next_index), current, true)?;
            next_index += 1;
        }
    }
    Ok(JsValue::new(*result))
}

pub fn array_map(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(array = stack, args.this.to_object(ctx)?);
    let length = super::get_length(ctx, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(ctx.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    letroot!(callback = stack, callback.to_object(ctx)?);
    letroot!(cb2 = stack, *callback);
    letroot!(result = stack, JsArray::new(ctx, 0));
    letroot!(this_arg = stack, args.at(1));
    let mut buf = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if !array.has_own_property(ctx, Symbol::Index(i)) {
            continue;
        }

        buf[0] = array.get(ctx, Symbol::Index(i))?;
        buf[1] = JsValue::new(i);
        buf[2] = JsValue::new(*array);
        let mut args = Arguments::new(*this_arg, &mut buf);
        let mapped_value = callback
            .as_function_mut()
            .call(ctx, &mut args, JsValue::new(*cb2))?;
        result.put(ctx, Symbol::Index(i), mapped_value, true)?;
    }
    Ok(JsValue::new(*result))
}

pub fn array_index_of(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(array = stack, args.this.to_object(ctx)?);
    let length = super::get_length(ctx, &mut array)?;

    let target = args.at(0);
    let from_index = if args.size() == 1 {
        0.0
    } else {
        args.at(1).to_interger(ctx)?
    };
    if from_index.is_infinite() {
        return Ok(JsValue::new(-1));
    }

    let from_index = from_index as u32;

    for i in from_index..length {
        if !array.has_own_property(ctx, Symbol::Index(i)) {
            continue;
        }

        let elem = array.get(ctx, Symbol::Index(i))?;
        if elem == target {
            return Ok(JsValue::new(i));
        }
    }
    Ok(JsValue::new(-1))
}

pub fn array_slice(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(obj = stack, args.this.to_object(ctx)?);

    let len = super::get_length(ctx, &mut obj)?;
    let mut k;
    if args.size() != 0 {
        let relative_start = args.at(0).to_int32(ctx)?;
        if relative_start < 0 {
            k = (relative_start + len as i32).max(0) as u32;
        } else {
            k = (relative_start as u32).min(len);
        }
    } else {
        k = 0;
    }

    let mut fin;
    if args.size() > 1 {
        if args.at(1).is_undefined() {
            fin = len;
        } else {
            let relative_end = args.at(1).to_int32(ctx)?;
            if unlikely(relative_end as u32 == 4294967295) {
                let msg = JsString::new(ctx, "Out of memory for array values");
                return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
            }
            if relative_end < 0 {
                fin = (relative_end + len as i32).max(0) as u32;
            } else {
                fin = (relative_end as u32).min(len);
            }
        }
    } else {
        fin = len;
    }

    let result_len = if fin > k { fin - k } else { 0 };
    if unlikely(result_len as u32 == 4294967295 || len >= 4294967295) {
        let msg = JsString::new(ctx, "Out of memory for array values");
        return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
    }
    if result_len > (1024 << 6) {
        letroot!(ary = stack, JsArray::new(ctx, result_len));

        let mut n = 0;
        while k < fin {
            let kval = obj.get(ctx, Symbol::Index(k))?;
            ary.define_own_property(
                ctx,
                Symbol::Index(n),
                &*DataDescriptor::new(kval, W | E | C),
                false,
            )?;
            k += 1;
            n += 1;
        }
        return Ok(JsValue::new(*ary));
    }
    letroot!(ary = stack, JsArray::new(ctx, result_len));
    let mut n = 0;
    while k < fin {
        if obj.has_property(ctx, Symbol::Index(k)) {
            let val = obj.get(ctx, Symbol::Index(k))?;
            ary.put(ctx, Symbol::Index(n), val, false)?;
        }
        k += 1;
        n += 1;
    }
    return Ok(JsValue::new(*ary));
}

pub fn array_shift(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(ctx)?;

    let length = super::get_length(ctx, &mut obj)?;
    if length == 0 {
        obj.put(ctx, "length".intern(), JsValue::new(0), false)?;
        return Ok(JsValue::encode_undefined_value());
    }

    let first = obj.get(ctx, Symbol::Index(0))?;
    for k in 1..length {
        let from = k;
        let to = k.wrapping_sub(1);
        let from_value = obj.get(ctx, Symbol::Index(from as u32))?;
        if from_value.is_undefined() {
            obj.delete(ctx, Symbol::Index(to as _), false)?;
        } else {
            obj.put(ctx, Symbol::Index(to as _), from_value, false)?;
        }
    }

    let final_index = length.wrapping_sub(1);
    obj.delete(ctx, Symbol::Index(final_index as _), false)?;
    obj.put(
        ctx,
        "length".intern(),
        JsValue::new(final_index as u32),
        false,
    )?;
    Ok(first)
}

impl Builtin for JsArray {
    fn native_references() -> Vec<usize> {
        vec![
            JsArray::class() as *const _ as usize,
            array::array_ctor as usize,
            array::array_from as usize,
            array::array_is_array as usize,
            array::array_join as usize,
            array::array_of as usize,
            array::array_pop as usize,
            array::array_push as usize,
            array::array_reduce as usize,
            array::array_to_string as usize,
            array::array_concat as usize,
            array::array_for_each as _,
            array::array_filter as _,
            array::array_map as _,
            array::array_shift as _,
            array::array_slice as _,
            array::array_index_of as _,
        ]
    }

    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data.object_prototype.unwrap();
        let structure = Structure::new_indexed(ctx, None, true);
        ctx.global_data.array_structure = Some(structure);
        let structure = Structure::new_unique_indexed(ctx, Some(obj_proto), false);
        let mut prototype = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);
        ctx.global_data
            .array_structure
            .unwrap()
            .change_prototype_with_no_transition(prototype);
        let mut constructor = JsNativeFunction::new(ctx, S_CONSTURCTOR.intern(), array_ctor, 1);

        def_native_property!(ctx, constructor, prototype, prototype, NONE)?;
        def_native_method!(ctx, constructor, isArray, array_is_array, 1)?;
        def_native_method!(ctx, constructor, of, array_of, 1)?;
        def_native_method!(ctx, constructor, from, array_from, 1)?;
        def_native_property!(ctx, prototype, constructor, constructor, W | C)?;
        def_native_method!(ctx, prototype, join, array_join, 1, W | C | E)?;
        def_native_method!(ctx, prototype, toString, array_to_string, 1, W | C | E)?;
        def_native_method!(ctx, prototype, push, array_push, 1, W | C | E)?;
        def_native_method!(ctx, prototype, pop, array_pop, 1, W | C | E)?;
        def_native_method!(ctx, prototype, reduce, array_reduce, 1, W | C | E)?;
        def_native_method!(ctx, prototype, concat, array_concat, 1, W | C | E)?;
        def_native_method!(ctx, prototype, forEach, array_for_each, 1, W | C | E)?;
        def_native_method!(ctx, prototype, filter, array_filter, 1, W | C | E)?;
        def_native_method!(ctx, prototype, map, array_map, 1, W | C | E)?;
        def_native_method!(ctx, prototype, slice, array_slice, 1, W | C | E)?;
        def_native_method!(ctx, prototype, shift, array::array_shift, 0)?;
        def_native_method!(ctx, prototype, indexOf, array_index_of, 1, W | C | E)?;
        ctx.global_data.array_prototype = Some(prototype);

        let mut global_object = ctx.global_object();

        def_native_property!(ctx, global_object, Array, constructor, W | C)?;

        Ok(())
    }
}
