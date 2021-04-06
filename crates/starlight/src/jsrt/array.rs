use super::{get_length, object::object_to_string};
use crate::{
    vm::Runtime,
    vm::{
        arguments::*, array::*, attributes::*, error::*, object::*, property_descriptor::*,
        string::*, symbol_table::*, value::*,
    },
};
pub fn array_ctor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let size = args.size();
    if size == 0 {
        return Ok(JsValue::encode_object_value(JsArray::new(vm, 0)));
    }
    if size == 1 {
        let first = args.at(1);
        if first.is_number() {
            let val = first.to_number(vm)?;
            let len = val as u32;
            if len as f64 == val {
                return Ok(JsValue::encode_object_value(JsArray::new(vm, len)));
            } else {
                let msg = JsString::new(vm, format!("invalid array length '{}", len));

                return Err(JsValue::encode_object_value(JsRangeError::new(
                    vm, msg, None,
                )));
            }
        } else {
            let mut ary = JsArray::new(vm, 1);
            ary.put(vm, Symbol::Index(0), first, false)?;

            Ok(JsValue::encode_object_value(ary))
        }
    } else {
        let mut ary = JsArray::new(vm, size as _);
        for i in 0..size {
            ary.define_own_property(
                vm,
                Symbol::Index(i as _),
                &*DataDescriptor::new(args.at(i), W | C | E),
                false,
            )?;
        }
        Ok(JsValue::encode_object_value(ary))
    }
}

pub fn array_is_array(_vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
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

pub fn array_of(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut ary = JsArray::new(vm, args.size() as _);
    for i in 0..args.size() {
        ary.put(vm, Symbol::Index(i as _), args.at(i), false)?;
    }
    Ok(JsValue::encode_object_value(ary))
}

pub fn array_from(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    root!(arg1 = stack, args.at(0).to_object(vm)?);
    let len = arg1.get(vm, "length".intern())?;
    let len = if len.is_number() {
        let n = len.to_number(vm)?;
        if n as u32 as f64 == n {
            n as u32
        } else {
            0
        }
    } else {
        0
    };
    let mut target = JsArray::new(vm, len);
    for k in 0..len {
        if arg1.has_property(vm, Symbol::Index(k)) {
            let value = arg1.get(vm, Symbol::Index(k))?;
            target.put(vm, Symbol::Index(k), value, false)?;
        }
    }

    Ok(JsValue::encode_object_value(target))
}
pub fn array_join(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    root!(obj = stack, args.this.to_object(vm)?);
    let len = obj.get(vm, "length".intern())?.to_number(vm)?;
    let len = if len as u32 as f64 == len {
        len as u32
    } else {
        0 as u32
    };
    let separator = if !args.at(0).is_undefined() {
        args.at(0).to_string(vm)?
    } else {
        ",".to_string()
    };

    let mut fmt = String::new();
    {
        let element0 = obj.get(vm, Symbol::Index(0))?;
        if !(element0.is_undefined() || element0.is_null()) {
            let str = element0.to_string(vm)?;
            fmt.push_str(&str);
        }
    }

    let mut k: u32 = 1;
    while k < len {
        fmt.push_str(&separator);
        let element = obj.get(vm, Symbol::Index(k))?;
        if !(element.is_undefined() || element.is_null()) {
            let str = element.to_string(vm)?;
            fmt.push_str(&str);
        }
        k += 1;
    }
    Ok(JsValue::encode_object_value(JsString::new(vm, fmt)))
}
pub fn array_to_string(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = vm.shadowstack();
    root!(this = stack, args.this.to_object(vm)?);
    let m = this.get_property(vm, "join".intern());
    if m.value().is_callable() {
        root!(func = stack, unsafe {
            m.value().get_object().downcast_unchecked::<JsObject>()
        });
        root!(f2 = stack, *&*func);
        let f = func.as_function_mut();
        root!(args = stack, Arguments::new(args.this, &mut []));
        return f.call(vm, &mut args, JsValue::new(*f2));
    }
    root!(args = stack, Arguments::new(args.this, &mut []));
    object_to_string(vm, &mut args)
}

// TODO(playX): Allow to push up to 2^53-1 values
pub fn array_push(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(vm)?;
    let n = obj.get(vm, "length".intern())?.to_number(vm)?;
    let mut n = if n as u32 as f64 == n {
        n as u32 as u64
    } else {
        let msg = JsString::new(vm, "invalid length");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            vm, msg, None,
        )));
    };
    // let p = n;
    let max = 0x100000000u64;
    let mut it = 0;
    let last = args.size();
    if (n + args.size() as u64) <= max {
        while it != last {
            obj.put(vm, Symbol::Index(n as _), args.at(it), false)?;
            it += 1;
            n += 1;
        }
    } else {
        let msg = JsString::new(vm, "array size exceeded");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            vm, msg, None,
        )));
    }
    let len = n as f64;
    obj.put(vm, "length".intern(), JsValue::new(len), false)?;
    Ok(JsValue::new(n as f64))
}

pub fn array_pop(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(vm)?;
    let n = obj.get(vm, "length".intern())?.to_number(vm)?;
    let len = if n as u32 as f64 == n {
        n as u32
    } else {
        let msg = JsString::new(vm, "invalid length");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            vm, msg, None,
        )));
    };
    if len == 0 {
        obj.put(vm, "length".intern(), JsValue::new(0.0), true)?;
        return Ok(JsValue::encode_undefined_value());
    } else {
        let index = len - 1;
        let element = obj.get(vm, Symbol::Index(index))?;
        obj.delete(vm, Symbol::Index(index), true)?;
        obj.put(vm, "length".intern(), JsValue::new(index as i32), true)?;
        Ok(element)
    }
}

pub fn array_reduce(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    root!(obj = stack, args.this.to_object(rt)?);
    let len = get_length(rt, &mut obj)?;
    let arg_count = args.size();
    if arg_count == 0 || !args.at(0).is_callable() {
        let msg = JsString::new(
            rt,
            "Array.prototype.reduce requires callable object as 1st argument",
        );
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    }

    root!(callbackf = stack, args.at(0).get_jsobject());
    root!(cb = stack, *&*callbackf);
    let callback = callbackf.as_function_mut();
    if len == 0 && arg_count <= 1 {
        let msg = JsString::new(
            rt,
            "Array.prototype.reduce with empty array requires initial value as 2nd argumentt",
        );
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    }
    let mut k = 0;
    root!(acc = stack, JsValue::encode_undefined_value());
    if arg_count > 1 {
        *acc = args.at(1);
    } else {
        let mut k_present = false;
        while k < len {
            if obj.has_property(rt, Symbol::Index(k)) {
                k_present = true;
                *acc = obj.get(rt, Symbol::Index(k))?;
                k += 1;
                break;
            }
            k += 1;
        }

        if !k_present {
            let msg = JsString::new(
                rt,
                "Array.prototype.reduce with empty array requires initial value",
            );
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }
    }

    while k < len {
        if obj.has_property(rt, Symbol::Index(k)) {
            let mut tmp = [JsValue::encode_undefined_value(); 4];
            root!(
                args = stack,
                Arguments::new(JsValue::encode_undefined_value(), &mut tmp)
            );
            *args.at_mut(0) = *acc;
            *args.at_mut(1) = obj.get(rt, Symbol::Index(k))?;
            *args.at_mut(2) = JsValue::new(k as i32);
            *args.at_mut(3) = JsValue::encode_object_value(*obj);
            *acc = callback.call(rt, &mut args, JsValue::new(*cb))?;
        }
        k += 1;
    }
    Ok(*acc)
}

pub fn array_concat(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(args.this);
    }

    let mut ix = 0;
    if !args.this.is_jsobject() {
        let msg = JsString::new(rt, "Array.prototype.concat requires array-like object");
        return Err(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )));
    }
    let stack = rt.shadowstack();
    root!(this = stack, args.this.get_jsobject());
    let this_length = super::get_length(rt, &mut this)?;

    let mut new_values = JsArray::new(rt, this_length);
    for n in 0..this_length {
        let val = this.get(rt, Symbol::Index(n))?;
        new_values.put(rt, Symbol::Index(ix), val, false)?;
        ix += 1;
    }

    for ai in 0..args.size() {
        let arg = args.at(ai);
        if !arg.is_jsobject() {
            let msg = JsString::new(rt, "Array.prototype.concat requires array-like arguments");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }
        root!(arg = stack, arg.get_jsobject());
        let len = super::get_length(rt, &mut arg)?;
        for n in 0..len {
            let val = arg.get(rt, Symbol::Index(n))?;
            new_values.put(rt, Symbol::Index(ix), val, false)?;
            ix += 1;
        }
    }

    Ok(JsValue::encode_object_value(new_values))
}

pub fn array_for_each(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    root!(array = stack, args.this.to_object(rt)?);
    let length = super::get_length(rt, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(rt.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    root!(callback = stack, callback.to_object(rt)?);
    root!(cb2 = stack, *&*callback);
    let this_arg = args.at(1);
    let mut buf: [JsValue; 3] = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if array.has_property(rt, Symbol::Index(i)) {
            let element = array.get(rt, Symbol::Index(i))?;
            buf[0] = element;
            buf[1] = JsValue::new(i);
            buf[2] = JsValue::new(*&*array);
            root!(args = stack, Arguments::new(this_arg, &mut buf));

            callback
                .as_function_mut()
                .call(rt, &mut args, JsValue::new(*cb2))?;
        }
    }
    Ok(JsValue::encode_undefined_value())
}

pub fn array_filter(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    root!(array = stack, args.this.to_object(rt)?);
    let length = super::get_length(rt, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(rt.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    root!(callback = stack, callback.to_object(rt)?);
    root!(cb2 = stack, *&*callback);
    root!(result = stack, JsArray::new(rt, 0));
    root!(this_arg = stack, args.at(1));

    let mut next_index = 0;
    let mut buf = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if !array.has_own_property(rt, Symbol::Index(i)) {
            continue;
        }
        let current = array.get(rt, Symbol::Index(i))?;
        buf[0] = current;
        buf[1] = JsValue::new(i);
        buf[2] = JsValue::new(*&*array);
        let mut args = Arguments::new(*&*this_arg, &mut buf);
        let val = callback
            .as_function_mut()
            .call(rt, &mut args, JsValue::new(*cb2))?;
        if val.to_boolean() {
            result.put(rt, Symbol::Index(next_index), current, true)?;
            next_index += 1;
        }
    }
    Ok(JsValue::new(*&*result))
}

pub fn array_map(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    root!(array = stack, args.this.to_object(rt)?);
    let length = super::get_length(rt, &mut array)?;

    let callback = args.at(0);
    if !callback.is_callable() {
        return Err(JsValue::new(rt.new_type_error(
            "Array.prototype.forEach callback must be a function",
        )));
    }

    root!(callback = stack, callback.to_object(rt)?);
    root!(cb2 = stack, *&*callback);
    root!(result = stack, JsArray::new(rt, 0));
    root!(this_arg = stack, args.at(1));
    let mut buf = [JsValue::encode_undefined_value(); 3];
    for i in 0..length {
        if !array.has_own_property(rt, Symbol::Index(i)) {
            continue;
        }

        buf[0] = array.get(rt, Symbol::Index(i))?;
        buf[1] = JsValue::new(i);
        buf[2] = JsValue::new(*&*array);
        let mut args = Arguments::new(*&*this_arg, &mut buf);
        let mapped_value = callback
            .as_function_mut()
            .call(rt, &mut args, JsValue::new(*cb2))?;
        result.put(rt, Symbol::Index(i), mapped_value, true)?;
    }
    Ok(JsValue::new(*&*result))
}

pub fn array_slice(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    root!(obj = stack, args.this.to_object(rt)?);

    let len = super::get_length(rt, &mut obj)?;
    let mut k;
    if args.size() != 0 {
        let relative_start = args.at(0).to_int32(rt)?;
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
            let relative_end = args.at(1).to_int32(rt)?;
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

    if result_len > 1024 << 6 {
        root!(ary = stack, JsArray::new(rt, result_len));

        let mut n = 0;
        while k < fin {
            let kval = obj.get(rt, Symbol::Index(k))?;
            ary.define_own_property(
                rt,
                Symbol::Index(n),
                &*DataDescriptor::new(kval, W | E | C),
                false,
            )?;
            k += 1;
            n += 1;
        }
        return Ok(JsValue::new(*&*ary));
    }
    root!(ary = stack, JsArray::new(rt, result_len));
    let mut n = 0;
    while k < fin {
        if obj.has_property(rt, Symbol::Index(k)) {
            let val = obj.get(rt, Symbol::Index(k))?;
            ary.put(rt, Symbol::Index(n), val, false)?;
        }
        k += 1;
        n += 1;
    }
    return Ok(JsValue::new(*&*ary));
}
