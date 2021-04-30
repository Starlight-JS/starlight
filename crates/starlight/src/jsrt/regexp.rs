use crate::{define_jsclass_with_symbol, prelude::*};
use regress::Regex;
use std::{
    intrinsics::unlikely,
    mem::{size_of, ManuallyDrop},
};

/// The internal representation on a `RegExp` object.
#[derive(Debug, Clone)]
pub struct RegExp {
    /// Regex matcher.
    matcher: Regex,

    /// Update last_index, set if global or sticky flags are set.
    use_last_index: bool,

    /// String of parsed flags.
    pub(crate) flags: Box<str>,

    /// Flag 's' - dot matches newline characters.
    dot_all: bool,

    /// Flag 'g'
    global: bool,

    /// Flag 'i' - ignore case.
    ignore_case: bool,

    /// Flag 'm' - '^' and '$' match beginning/end of line.
    multiline: bool,

    /// Flag 'y'
    sticky: bool,

    /// Flag 'u' - Unicode.
    unicode: bool,

    pub(crate) original_source: Box<str>,
    pub(crate) original_flags: Box<str>,
}
extern "C" fn drop_regexp_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<RegExp>()) }
}

extern "C" fn deser(obj: &mut JsObject, deser: &mut Deserializer, _rt: &mut Runtime) {
    unsafe {
        let use_last_index = bool::deserialize_inplace(deser);
        let flags = String::deserialize_inplace(deser);
        let dot_all = bool::deserialize_inplace(deser);
        let global = bool::deserialize_inplace(deser);
        let ignore_case = bool::deserialize_inplace(deser);
        let multiline = bool::deserialize_inplace(deser);
        let sticky = bool::deserialize_inplace(deser);
        let unicode = bool::deserialize_inplace(deser);
        let original_source = String::deserialize_inplace(deser);
        let original_flags = String::deserialize_inplace(deser);

        let mut sorted_flags = String::new();
        if original_flags.contains('g') {
            sorted_flags.push('g');
        }
        if original_flags.contains('i') {
            sorted_flags.push('i');
        }
        if original_flags.contains('m') {
            sorted_flags.push('m');
        }
        if original_flags.contains('s') {
            sorted_flags.push('s');
        }
        if original_flags.contains('u') {
            sorted_flags.push('u');
        }
        if original_flags.contains('y') {
            sorted_flags.push('y');
        }
        let matcher = Regex::with_flags(&original_source, sorted_flags.as_str()).unwrap();
        *obj.data::<RegExp>() = ManuallyDrop::new(RegExp {
            use_last_index,
            flags: flags.into_boxed_str(),
            dot_all,
            global,
            ignore_case,
            multiline,
            sticky,
            unicode,
            original_source: original_source.into_boxed_str(),
            original_flags: original_flags.into_boxed_str(),
            matcher,
        });
    }
}
extern "C" fn ser(obj: &JsObject, serializer: &mut SnapshotSerializer) {
    let data = obj.data::<RegExp>();
    data.use_last_index.serialize(serializer);
    data.flags.to_string().serialize(serializer);
    data.dot_all.serialize(serializer);
    data.global.serialize(serializer);
    data.ignore_case.serialize(serializer);
    data.multiline.serialize(serializer);
    data.sticky.serialize(serializer);
    data.unicode.serialize(serializer);
    data.original_source.to_string().serialize(serializer);
    data.original_flags.to_string().serialize(serializer);
}
extern "C" fn fsz() -> usize {
    size_of::<RegExp>()
}
impl RegExp {
    define_jsclass_with_symbol!(
        JsObject,
        RegExp,
        Object,
        Some(drop_regexp_fn),
        None,
        Some(deser),
        Some(ser),
        Some(fsz)
    );

    pub(crate) fn init(rt: &mut Runtime, obj_proto: GcPointer<JsObject>) {
        rt.global_data.regexp_structure = Some(Structure::new_indexed(rt, None, false));
        let proto_map = rt
            .global_data
            .regexp_structure
            .unwrap()
            .change_prototype_with_no_transition(obj_proto);
        let mut init = || -> Result<(), JsValue> {
            let mut proto =
                JsObject::new(rt, &proto_map, JsObject::get_class(), ObjectTag::Ordinary);

            let mut constructor =
                JsNativeFunction::new(rt, "RegExp".intern(), regexp_constructor, 2);

            rt.global_object()
                .put(rt, "RegExp".intern(), JsValue::new(constructor), false)?;

            constructor.put(rt, "prototype".intern(), JsValue::new(proto), false)?;

            proto.put(rt, "constructor".intern(), JsValue::new(constructor), false)?;
            def_native_method!(rt, proto, exec, regexp_exec, 1)?;
            def_native_method!(rt, proto, test, regexp_test, 1)?;
            def_native_method!(rt, proto, toString, regexp_to_string, 0)?;
            let mut sym = rt
                .global_object()
                .get(rt, "Symbol".intern())?
                .get_jsobject();
            let sym_match = sym.get(rt, "match".intern())?.to_symbol(rt)?;
            let f = JsNativeFunction::new(rt, sym_match, regexp_match, 1);
            proto.put(rt, sym_match, JsValue::new(f), false)?;
            rt.global_data.regexp_object = Some(proto);
            Ok(())
        };

        match init() {
            Err(_) => unreachable!(),
            _ => (),
        }
    }
}

pub fn regexp_constructor(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let proto = rt.global_data().regexp_object.unwrap();
    let structure = Structure::new_indexed(rt, Some(proto), false);

    let arg = args.at(0);

    let (regex_body, mut regex_flags) = match arg {
        arg if arg.is_jsstring() => (
            arg.to_string(rt)?.into_boxed_str(),
            String::new().into_boxed_str(),
        ),
        arg if arg.is_jsobject() => {
            let obj = arg.get_jsobject();
            if obj.is_class(RegExp::get_class()) {
                (
                    obj.data::<RegExp>().original_source.clone(),
                    obj.data::<RegExp>().original_flags.clone(),
                )
            } else {
                (
                    String::new().into_boxed_str(),
                    String::new().into_boxed_str(),
                )
            }
        }
        _ => return Err(JsValue::encode_undefined_value()),
    };

    if args.at(1).is_jsstring() {
        regex_flags = args
            .at(1)
            .get_jsstring()
            .as_str()
            .to_owned()
            .into_boxed_str();
    }

    let mut sorted_flags = String::new();
    let mut dot_all = false;
    let mut global = false;
    let mut ignore_case = false;
    let mut multiline = false;
    let mut sticky = false;
    let mut unicode = false;
    if regex_flags.contains('g') {
        global = true;
        sorted_flags.push('g');
    }
    if regex_flags.contains('i') {
        ignore_case = true;
        sorted_flags.push('i');
    }
    if regex_flags.contains('m') {
        multiline = true;
        sorted_flags.push('m');
    }
    if regex_flags.contains('s') {
        dot_all = true;
        sorted_flags.push('s');
    }
    if regex_flags.contains('u') {
        unicode = true;
        sorted_flags.push('u');
    }
    if regex_flags.contains('y') {
        sticky = true;
        sorted_flags.push('y');
    }

    let matcher = match Regex::with_flags(&regex_body, sorted_flags.as_str()) {
        Err(error) => {
            let msg = JsString::new(
                rt,
                format!("failed to create matcher: {} in {}", error.text, regex_body),
            );
            return Err(JsValue::new(JsSyntaxError::new(rt, msg, None)));
        }
        Ok(val) => val,
    };

    let regexp = RegExp {
        matcher,
        use_last_index: global || sticky,
        flags: sorted_flags.clone().into_boxed_str(),
        dot_all,
        global,
        ignore_case,
        multiline,
        sticky,
        unicode,
        original_source: regex_body,
        original_flags: regex_flags,
    };
    let mut this = JsObject::new(rt, &structure, RegExp::get_class(), ObjectTag::Regex);
    *this.data::<RegExp>() = ManuallyDrop::new(regexp);
    let f = JsString::new(rt, sorted_flags);
    this.put(rt, "flags".intern(), JsValue::new(f), false)?;
    this.put(rt, "global".intern(), JsValue::new(global), false)?;
    this.put(rt, "unicode".intern(), JsValue::new(unicode), false)?;

    Ok(JsValue::new(this))
}

pub fn regexp_test(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(!args.this.is_jsobject()) {
        return Err(JsValue::new(rt.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let mut this = args.this.get_jsobject();
    let mut last_index = this.get(rt, "lastIndex".intern())?.to_int32(rt)? as usize;

    let arg_str = args.at(0).to_string(rt)?;
    if this.is_class(RegExp::get_class()) {
        let result = if let Some(m) = this
            .data::<RegExp>()
            .matcher
            .find_from(arg_str.as_str(), last_index)
            .next()
        {
            if this.data::<RegExp>().use_last_index {
                last_index = m.end();
            }
            true
        } else {
            if this.data::<RegExp>().use_last_index {
                last_index = 0;
            }
            false
        };
        this.put(
            rt,
            "lastIndex".intern(),
            JsValue::new(last_index as u32),
            false,
        )?;
        return Ok(JsValue::new(result));
    } else {
        return Err(JsValue::new(rt.new_type_error(
            "RegExp.prototype.test method called on incompatible value",
        )));
    }
}

pub fn regexp_exec(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(!args.this.is_jsobject()) {
        return Err(JsValue::new(rt.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let mut this = args.this.get_jsobject();
    let mut last_index = this.get(rt, "lastIndex".intern())?.to_int32(rt)? as usize;
    let mut obj = this;
    if unlikely(!this.is_class(RegExp::get_class())) {
        return Err(JsValue::new(rt.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let regex = obj.data::<RegExp>();
    let arg_str = args.at(0).to_string(rt)?;
    let result = if let Some(m) = regex.matcher.find_from(arg_str.as_str(), last_index).next() {
        if regex.use_last_index {
            last_index = m.end();
        }
        let groups = m.captures.len() + 1;
        let mut result = Vec::with_capacity(groups);
        for i in 0..groups {
            if let Some(range) = m.group(i) {
                result.push(JsValue::new(JsString::new(
                    rt,
                    arg_str.get(range).expect("Could not get slice"),
                )));
            } else {
                result.push(JsValue::encode_undefined_value());
            }
        }
        let v = result;
        let mut result = JsArray::new(rt, v.len() as _);
        for i in 0..v.len() {
            result.put(rt, Symbol::Index(i as _), v[i], false)?;
        }
        result.define_own_property(
            rt,
            "index".intern(),
            &*DataDescriptor::new(JsValue::new(m.start() as u32), W | C | E),
            false,
        )?;
        let input = JsValue::new(JsString::new(rt, arg_str));
        result.put(rt, "input".intern(), input, false)?;

        JsValue::new(result)
    } else {
        if regex.use_last_index {
            last_index = 0;
        }
        JsValue::encode_null_value()
    };
    this.put(
        rt,
        "lastIndex".intern(),
        JsValue::new(last_index as u32),
        false,
    )?;

    Ok(result)
}

fn to_regexp(val: JsValue) -> Option<GcPointer<JsObject>> {
    if val.is_jsobject() && val.get_jsobject().is_class(RegExp::get_class()) {
        return Some(val.get_jsobject());
    }
    None
}

pub fn regexp_to_string(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    match to_regexp(args.this) {
        Some(object) => {
            let regex = object.data::<RegExp>();

            Ok(JsValue::new(JsString::new(
                rt,
                format!("/{}/{}", regex.original_source, regex.flags),
            )))
        }
        None => Err(JsValue::new(
            rt.new_type_error("RegExp.prototype.toString is not generic"),
        )),
    }
}

/// @@match
pub fn regexp_match(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg_str = args.at(0).to_string(rt)?;
    let matches = if let Some(object) = to_regexp(args.this) {
        let regex = object.data::<RegExp>();
        let mut matches = vec![];
        for mat in regex.matcher.find_iter(&arg_str) {
            let match_vec: Vec<JsValue> = mat
                .groups()
                .map(|group| match group {
                    Some(range) => JsValue::new(JsString::new(rt, &arg_str[range])),
                    None => JsValue::encode_undefined_value(),
                })
                .collect();

            let mut match_val = JsArray::from_slice(rt, &match_vec);
            match_val.put(
                rt,
                "index".intern(),
                JsValue::new(mat.start() as u32),
                false,
            )?;
            let input = JsString::new(rt, arg_str.clone());
            match_val.put(rt, "input".intern(), JsValue::new(input), false)?;
            matches.push(JsValue::new(match_val));
            if !regex.flags.contains('g') {
                break;
            }
        }

        matches
    } else {
        return Err(JsValue::new(
            rt.new_type_error("RegExp.prototype.@@match is not generic"),
        ));
    };

    let result = JsArray::from_slice(rt, &matches);
    Ok(JsValue::new(result))
}
