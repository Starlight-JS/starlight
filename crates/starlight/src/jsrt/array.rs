use crate::{
    runtime::{
        arguments::Arguments, array::JsArray, attributes::*, error::JsRangeError,
        object::ObjectTag, property_descriptor::DataDescriptor, string::JsString, symbol::Symbol,
        value::JsValue,
    },
    vm::VirtualMachine,
};

use super::object::object_to_string;

pub fn array_ctor(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let size = args.size();
    if size == 0 {
        return Ok(JsValue::new(JsArray::new(vm, 0)));
    }
    if size == 1 {
        let first = args.at(1);
        if first.is_number() {
            let val = first.to_number(vm)?;
            let len = val as u32;
            if len as f64 == val {
                return Ok(JsValue::new(JsArray::new(vm, len)));
            } else {
                let msg =
                    JsString::new(vm, format!("invalid array length '{}", len)).root(vm.space());

                return Err(JsValue::new(JsRangeError::new(vm, *msg, None)));
            }
        } else {
            let mut ary = JsArray::new(vm, 1);
            ary.put(vm, Symbol::Indexed(0), first, false)?;

            Ok(JsValue::new(ary))
        }
    } else {
        let mut ary = JsArray::new(vm, size as _).root(vm.space());
        for i in 0..size {
            ary.define_own_property(
                vm,
                Symbol::Indexed(i as _),
                &*DataDescriptor::new(args.at(i), W | C | E),
                false,
            )?;
        }
        Ok(JsValue::new(*ary))
    }
}

pub fn array_is_array(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::new(false));
    }
    let val = args.at(0);
    if !val.is_object() {
        return Ok(JsValue::new(false));
    }
    Ok(JsValue::new(val.as_object().tag() == ObjectTag::Array))
}

pub fn array_of(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut ary = JsArray::new(vm, args.size() as _).root(vm.space());
    for i in 0..args.size() {
        ary.put(vm, Symbol::Indexed(i as _), args.at(i), false)?;
    }
    Ok(JsValue::new(*ary))
}

pub fn array_from(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut arg1 = args.at(0).to_object(vm)?.root(vm.space());
    let len = arg1.get(vm, Symbol::length())?;
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
    let mut target = JsArray::new(vm, len).root(vm.space());
    for k in 0..len {
        if arg1.has_property(vm, Symbol::Indexed(k)) {
            let value = arg1.get(vm, Symbol::Indexed(k))?;
            target.put(vm, Symbol::Indexed(k), value, false)?;
        }
    }

    Ok(JsValue::new(*target))
}
pub fn array_join(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let mut obj = args.this.to_object(vm)?.root(vm.space());
    let len = obj.get(vm, Symbol::length())?.to_number(vm)?;
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
        let element0 = obj.get(vm, Symbol::Indexed(0))?;
        if !element0.is_undefined_or_null() {
            let str = element0.to_string(vm)?;
            fmt.push_str(&str);
        }
    }

    let mut k: u32 = 1;
    while k < len {
        fmt.push_str(&separator);
        let element = obj.get(vm, Symbol::Indexed(k))?;
        if !element.is_undefined_or_null() {
            let str = element.to_string(vm)?;
            fmt.push_str(&str);
        }
        k += 1;
    }
    Ok(JsValue::new(JsString::new(vm, fmt)))
}
pub fn array_to_string(vm: &mut VirtualMachine, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(vm)?.root(vm.space());
    let m = this.get_property(vm, Symbol::join());
    if m.value().is_callable() {
        let mut f = m.value().as_object();
        let f = f.as_function_mut();
        let mut args = Arguments::new(vm, args.this, 0);
        return f.call(vm, &mut args);
    }
    let mut args = Arguments::new(vm, args.this, 0);
    object_to_string(vm, &mut args)
}
