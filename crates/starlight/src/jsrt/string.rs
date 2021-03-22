use crate::{
    gc::cell::GcPointer,
    vm::{
        arguments::Arguments,
        array::JsArray,
        attributes::*,
        error::JsTypeError,
        function::JsNativeFunction,
        object::JsObject,
        property_descriptor::DataDescriptor,
        string::{JsString, JsStringObject},
        structure::Structure,
        symbol_table::{Internable, Symbol},
        value::*,
        Runtime,
    },
};

pub fn string_to_string(_rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn string_concat(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let val = args.this;
    val.check_object_coercible(rt)?;
    let mut str = val.to_string(rt)?;
    for i in 0..args.size() {
        let arg = args.at(i);
        let r = arg.to_string(rt)?;
        str.push_str(&r);
    }
    Ok(JsValue::encode_object_value(JsString::new(rt, str)))
}

pub fn string_value_of(_rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn string_split(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let val = args.this;
    val.check_object_coercible(rt)?;
    let str = val.to_string(rt)?;

    let argc = args.size();
    let lim = if argc < 2 || args.at(1).is_undefined() {
        4294967295u32
    } else {
        args.at(1).to_uint32(rt)?
    };
    let separator = if args.at(0).is_undefined() || args.at(0).is_null() {
        None
    } else {
        Some(args.at(0).to_string(rt)?)
    };
    let values = match separator {
        None if lim == 0 => vec![],
        None => vec![JsValue::encode_object_value(JsString::new(rt, str))],
        Some(separator) if separator.is_empty() => str
            .encode_utf16()
            .map(|cp| {
                JsValue::encode_object_value(JsString::new(rt, String::from_utf16_lossy(&[cp])))
            })
            .take(lim as _)
            .collect(),
        Some(separator) => str
            .split(separator.as_str())
            .map(|x| JsValue::encode_object_value(JsString::new(rt, x)))
            .take(lim as _)
            .collect(),
    };

    let mut arr = JsArray::new(rt, values.len() as _);
    for (ix, item) in values.iter().enumerate() {
        arr.put(rt, Symbol::Index(ix as _), *item, false)?;
    }
    Ok(JsValue::encode_object_value(arr))
}

pub fn string_constructor(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.ctor_call {
        let str;
        if args.size() != 0 {
            str = args.at(0).to_string(rt)?;
        } else {
            str = "".to_owned();
        }
        let _ = str;
        let msg = JsString::new(
            rt,
            "String.prototype.constructor as constructor is not yet implemented",
        );
        Ok(JsValue::encode_object_value(JsTypeError::new(
            rt, msg, None,
        )))
    } else {
        if args.size() != 0 {
            let str = args.at(0).to_string(rt)?;
            let jsttr = JsString::new(rt, str);
            return Ok(JsValue::encode_object_value(jsttr));
        } else {
            let jsttr = JsString::new(rt, "");
            return Ok(JsValue::encode_object_value(jsttr));
        }
    }
}

pub(super) fn initialize(rt: &mut Runtime, obj_proto: GcPointer<JsObject>) {
    rt.global_data.string_structure = Some(Structure::new_indexed(rt, None, true));
    let map = Structure::new_unique_with_proto(rt, Some(obj_proto), false);
    let mut proto = JsStringObject::new_plain(rt, &map);

    rt.global_data()
        .string_structure
        .unwrap()
        .change_prototype_with_no_transition(proto);
    let mut ctor = JsNativeFunction::new(rt, "String".intern(), string_constructor, 1);

    rt.global_object()
        .put(
            rt,
            "String".intern(),
            JsValue::encode_object_value(ctor),
            false,
        )
        .unwrap_or_else(|_| panic!());

    let _ = ctor.define_own_property(
        rt,
        "prototype".intern(),
        &*DataDescriptor::new(JsValue::from(proto), NONE),
        false,
    );

    proto
        .define_own_property(
            rt,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(ctor), W | C),
            false,
        )
        .unwrap_or_else(|_| panic!());
    let func = JsNativeFunction::new(rt, "toString".intern(), string_to_string, 0);
    proto
        .put(
            rt,
            "toString".intern(),
            JsValue::encode_object_value(func),
            false,
        )
        .unwrap_or_else(|_| panic!());
    let func = JsNativeFunction::new(rt, "valueOf".intern(), string_value_of, 0);
    proto
        .put(
            rt,
            "valueOf".intern(),
            JsValue::encode_object_value(func),
            false,
        )
        .unwrap_or_else(|_| panic!());

    let func = JsNativeFunction::new(rt, "split".intern(), string_split, 0);
    proto
        .put(
            rt,
            "split".intern(),
            JsValue::encode_object_value(func),
            false,
        )
        .unwrap_or_else(|_| panic!());

    let func = JsNativeFunction::new(rt, "concat".intern(), string_concat, 0);
    proto
        .put(
            rt,
            "concat".intern(),
            JsValue::encode_object_value(func),
            false,
        )
        .unwrap_or_else(|_| panic!());
    rt.global_data.string_prototype = Some(proto);
}
