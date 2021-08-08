use crate::constant::S_CONSTURCTOR;
use crate::js_method_table;
use crate::vm::builder::Builtin;
use crate::{define_jsclass, prelude::*, vm::context::Context};
use regress::Regex;
use std::{
    intrinsics::unlikely,
    mem::{size_of, ManuallyDrop},
};

/// The internal representation on a `RegExp` object.
#[derive(Debug, Clone)]
pub struct JsRegExp {
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
extern "C" fn drop_regexp_fn(obj: GcPointer<JsObject>) {
    unsafe { ManuallyDrop::drop(obj.data::<JsRegExp>()) }
}
/*
extern "C" fn deser(obj: &mut JsObject, deser: &mut Deserializer) {
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

        let mut soctxed_flags = String::new();
        if original_flags.contains('g') {
            soctxed_flags.push('g');
        }
        if original_flags.contains('i') {
            soctxed_flags.push('i');
        }
        if original_flags.contains('m') {
            soctxed_flags.push('m');
        }
        if original_flags.contains('s') {
            soctxed_flags.push('s');
        }
        if original_flags.contains('u') {
            soctxed_flags.push('u');
        }
        if original_flags.contains('y') {
            soctxed_flags.push('y');
        }
        let matcher = Regex::with_flags(&original_source, soctxed_flags.as_str()).unwrap();
        *obj.data::<JsRegExp>() = ManuallyDrop::new(JsRegExp {
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
    let data = obj.data::<JsRegExp>();
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
}*/
extern "C" fn fsz() -> usize {
    size_of::<JsRegExp>()
}

impl JsClass for JsRegExp {
    fn class() -> &'static Class {
        define_jsclass!(JsRegExp, RegExp, Some(drop_regexp_fn), None, Some(fsz))
    }
}

impl Builtin for JsRegExp {
    fn native_references() -> Vec<usize> {
        vec![
            regexp_constructor as _,
            regexp_exec as _,
            regexp_test as _,
            regexp_to_string as _,
            regexp_match as _,
            regexp_split_fast as _,
        ]
    }
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data.object_prototype.unwrap();

        ctx.global_data.regexp_structure = Some(Structure::new_indexed(ctx, None, false));
        let structure = ctx
            .global_data
            .regexp_structure
            .unwrap()
            .change_prototype_with_no_transition(obj_proto);

        let mut prototype = JsObject::new(ctx, &structure, JsObject::class(), ObjectTag::Ordinary);

        let mut constructor = JsNativeFunction::new(ctx, "RegExp".intern(), regexp_constructor, 2);

        constructor.put(ctx, "prototype".intern(), JsValue::new(prototype), false)?;

        prototype.put(
            ctx,
            S_CONSTURCTOR.intern(),
            JsValue::new(constructor),
            false,
        )?;
        def_native_method!(ctx, constructor, ___splitFast, regexp_split_fast, 3)?;
        def_native_method!(ctx, prototype, exec, regexp_exec, 1)?;
        def_native_method!(ctx, prototype, test, regexp_test, 1)?;
        def_native_method!(ctx, prototype, toString, regexp_to_string, 0)?;

        ctx.global_data.regexp_prototype = Some(prototype);

        ctx.global_object()
            .put(ctx, "RegExp", JsValue::new(constructor), false)?;

        let mut sym = ctx
            .global_object()
            .get(ctx, "Symbol".intern())?
            .get_jsobject();

        let sym_match = sym.get(ctx, "match".intern())?.to_symbol(ctx)?;
        let f = JsNativeFunction::new(ctx, sym_match, regexp_match, 1);
        prototype.put(ctx, sym_match, JsValue::new(f), false)?;
        Ok(())
    }
}

pub fn regexp_split_fast(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(!args.at(0).is_jsobject()) {
        return Err(JsValue::new(ctx.new_type_error(
            "Regex.@@splitFast requires regexp object as first argument",
        )));
    }
    let re = args.at(0).get_jsobject();
    let regexp = re.data::<JsRegExp>();
    if unlikely(!re.is_class(JsRegExp::class())) {
        return Err(JsValue::new(ctx.new_type_error(
            "Regex.@@splitFast requires regexp object as first argument",
        )));
    }
    let input = args.at(1).to_string(ctx)?;
    let limit = if args.at(2).is_undefined() {
        u32::MAX - 1
    } else {
        args.at(2).to_uint32(ctx)?
    };

    let mut result = JsArray::new(ctx, 0);
    //let mut result_length = 0;
    // let input_size = input.len();

    //let mut position = 0;
    if limit == 0 {
        return Ok(JsValue::new(result));
    }

    if input.is_empty() {
        let match_result = regexp.matcher.find(&input);
        if match_result.is_none() {
            let str = JsString::new(ctx, input);
            result.put(ctx, Symbol::Index(0), JsValue::new(str), false)?;
        }
        return Ok(JsValue::new(result));
    }
    //let mut match_position = position;
    // let regexp_is_sticky = regexp.sticky;
    //let regexp_is_unicode = regexp.unicode;
    let iter = input.splitn(limit as _, RegexPattern(&regexp.matcher));
    for (i, r) in iter.enumerate() {
        let str = JsString::new(ctx, r);
        result.put(ctx, Symbol::Index(i as _), JsValue::new(str), false)?;
    }
    Ok(JsValue::new(result))
}
pub fn regexp_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let proto = ctx.global_data.regexp_prototype.unwrap();
    let structure = Structure::new_indexed(ctx, Some(proto), false);

    let arg = args.at(0);

    let (regex_body, mut regex_flags) = match arg {
        arg if arg.is_jsstring() => (
            arg.to_string(ctx)?.into_boxed_str(),
            String::new().into_boxed_str(),
        ),
        arg if arg.is_jsobject() => {
            let obj = arg.get_jsobject();
            if obj.is_class(JsRegExp::class()) {
                (
                    obj.data::<JsRegExp>().original_source.clone(),
                    obj.data::<JsRegExp>().original_flags.clone(),
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

    let mut soctxed_flags = String::new();
    let mut dot_all = false;
    let mut global = false;
    let mut ignore_case = false;
    let mut multiline = false;
    let mut sticky = false;
    let mut unicode = false;
    if regex_flags.contains('g') {
        global = true;
        soctxed_flags.push('g');
    }
    if regex_flags.contains('i') {
        ignore_case = true;
        soctxed_flags.push('i');
    }
    if regex_flags.contains('m') {
        multiline = true;
        soctxed_flags.push('m');
    }
    if regex_flags.contains('s') {
        dot_all = true;
        soctxed_flags.push('s');
    }
    if regex_flags.contains('u') {
        unicode = true;
        soctxed_flags.push('u');
    }
    if regex_flags.contains('y') {
        sticky = true;
        soctxed_flags.push('y');
    }

    let matcher = match Regex::with_flags(&regex_body, soctxed_flags.as_str()) {
        Err(error) => {
            let msg = JsString::new(
                ctx,
                format!("failed to create matcher: {} in {}", error.text, regex_body),
            );
            return Err(JsValue::new(JsSyntaxError::new(ctx, msg, None)));
        }
        Ok(val) => val,
    };

    let regexp = JsRegExp {
        matcher,
        use_last_index: global || sticky,
        flags: soctxed_flags.clone().into_boxed_str(),
        dot_all,
        global,
        ignore_case,
        multiline,
        sticky,
        unicode,
        original_source: regex_body,
        original_flags: regex_flags,
    };
    let mut this = JsObject::new(ctx, &structure, JsRegExp::class(), ObjectTag::Regex);
    *this.data::<JsRegExp>() = ManuallyDrop::new(regexp);
    let f = JsString::new(ctx, soctxed_flags);
    this.put(ctx, "flags".intern(), JsValue::new(f), false)?;
    this.put(ctx, "global".intern(), JsValue::new(global), false)?;
    this.put(ctx, "unicode".intern(), JsValue::new(unicode), false)?;
    this.put(ctx, "lastIndex".intern(), JsValue::new(0), false)?;
    Ok(JsValue::new(this))
}

pub fn regexp_test(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(!args.this.is_jsobject()) {
        return Err(JsValue::new(ctx.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let mut this = args.this.get_jsobject();
    let mut last_index = this.get(ctx, "lastIndex".intern())?.to_int32(ctx)? as usize;

    let arg_str = args.at(0).to_string(ctx)?;
    if this.is_class(JsRegExp::class()) {
        let result = if let Some(m) = this
            .data::<JsRegExp>()
            .matcher
            .find_from(arg_str.as_str(), last_index)
            .next()
        {
            if this.data::<JsRegExp>().use_last_index {
                last_index = m.end();
            }
            true
        } else {
            if this.data::<JsRegExp>().use_last_index {
                last_index = 0;
            }
            false
        };
        this.put(
            ctx,
            "lastIndex".intern(),
            JsValue::new(last_index as u32),
            false,
        )?;
        return Ok(JsValue::new(result));
    } else {
        return Err(JsValue::new(ctx.new_type_error(
            "RegExp.prototype.test method called on incompatible value",
        )));
    }
}

pub fn regexp_exec(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if unlikely(!args.this.is_jsobject()) {
        return Err(JsValue::new(ctx.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let mut this = args.this.get_jsobject();
    let mut last_index = this.get(ctx, "lastIndex".intern())?.to_int32(ctx)? as usize;
    let mut obj = this;
    if unlikely(!this.is_class(JsRegExp::class())) {
        return Err(JsValue::new(ctx.new_type_error(
            "RegExp.prototype.exec method called on incompatible value",
        )));
    }
    let regex = obj.data::<JsRegExp>();
    let arg_str = args.at(0).to_string(ctx)?;
    let result = if let Some(m) = regex.matcher.find_from(arg_str.as_str(), last_index).next() {
        if regex.use_last_index {
            last_index = m.end();
        }
        let groups = m.captures.len() + 1;
        let mut result = Vec::with_capacity(groups);
        for i in 0..groups {
            if let Some(range) = m.group(i) {
                result.push(JsValue::new(JsString::new(
                    ctx,
                    arg_str.get(range).expect("Could not get slice"),
                )));
            } else {
                result.push(JsValue::encode_undefined_value());
            }
        }
        let v = result;
        let mut result = JsArray::new(ctx, v.len() as _);
        for i in 0..v.len() {
            result.put(ctx, Symbol::Index(i as _), v[i], false)?;
        }
        result.define_own_property(
            ctx,
            "index".intern(),
            &*DataDescriptor::new(JsValue::new(m.start() as u32), W | C | E),
            false,
        )?;
        let input = JsValue::new(JsString::new(ctx, arg_str));
        result.put(ctx, "input".intern(), input, false)?;

        JsValue::new(result)
    } else {
        if regex.use_last_index {
            last_index = 0;
        }
        JsValue::encode_null_value()
    };
    this.put(
        ctx,
        "lastIndex".intern(),
        JsValue::new(last_index as u32),
        false,
    )?;

    Ok(result)
}

fn to_regexp(val: JsValue) -> Option<GcPointer<JsObject>> {
    if val.is_jsobject() && val.get_jsobject().is_class(JsRegExp::class()) {
        return Some(val.get_jsobject());
    }
    None
}

pub fn regexp_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    match to_regexp(args.this) {
        Some(object) => {
            let regex = object.data::<JsRegExp>();

            Ok(JsValue::new(JsString::new(
                ctx,
                format!("/{}/{}", regex.original_source, regex.flags),
            )))
        }
        None => Err(JsValue::new(
            ctx.new_type_error("RegExp.prototype.toString is not generic"),
        )),
    }
}

/// @@match
pub fn regexp_match(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let arg_str = args.at(0).to_string(ctx)?;
    let matches = if let Some(object) = to_regexp(args.this) {
        let regex = object.data::<JsRegExp>();
        let mut matches = vec![];
        for mat in regex.matcher.find_iter(&arg_str) {
            let match_vec: Vec<JsValue> = mat
                .groups()
                .map(|group| match group {
                    Some(range) => JsValue::new(JsString::new(ctx, &arg_str[range])),
                    None => JsValue::encode_undefined_value(),
                })
                .collect();

            let mut match_val = JsArray::from_slice(ctx, &match_vec);
            match_val.put(
                ctx,
                "index".intern(),
                JsValue::new(mat.start() as u32),
                false,
            )?;
            let input = JsString::new(ctx, arg_str.clone());
            match_val.put(ctx, "input".intern(), JsValue::new(input), false)?;
            matches.push(JsValue::new(match_val));
            if !regex.flags.contains('g') {
                break;
            }
        }

        matches
    } else {
        return Err(JsValue::new(
            ctx.new_type_error("RegExp.prototype.@@match is not generic"),
        ));
    };

    let result = JsArray::from_slice(ctx, &matches);
    Ok(JsValue::new(result))
}

use std::str::pattern::{Pattern, SearchStep, Searcher};

use regress::Matches;

pub struct RegexSearcher<'r, 't> {
    haystack: &'t str,
    it: Matches<'r, 't>,
    last_step_end: usize,
    next_match: Option<(usize, usize)>,
}
pub struct RegexPattern<'r>(&'r Regex);
impl<'r, 't> Pattern<'t> for RegexPattern<'r> {
    type Searcher = RegexSearcher<'r, 't>;

    fn into_searcher(self, haystack: &'t str) -> RegexSearcher<'r, 't> {
        RegexSearcher {
            haystack,
            it: self.0.find_iter(haystack),
            last_step_end: 0,
            next_match: None,
        }
    }
}

unsafe impl<'r, 't> Searcher<'t> for RegexSearcher<'r, 't> {
    #[inline]
    fn haystack(&self) -> &'t str {
        self.haystack
    }

    #[inline]
    fn next(&mut self) -> SearchStep {
        if let Some((s, e)) = self.next_match {
            self.next_match = None;
            self.last_step_end = e;
            return SearchStep::Match(s, e);
        }
        match self.it.next() {
            None => {
                if self.last_step_end < self.haystack().len() {
                    let last = self.last_step_end;
                    self.last_step_end = self.haystack().len();
                    SearchStep::Reject(last, self.haystack().len())
                } else {
                    SearchStep::Done
                }
            }
            Some(m) => {
                let (s, e) = (m.start(), m.end());
                if s == self.last_step_end {
                    self.last_step_end = e;
                    SearchStep::Match(s, e)
                } else {
                    self.next_match = Some((s, e));
                    let last = self.last_step_end;
                    self.last_step_end = s;
                    SearchStep::Reject(last, s)
                }
            }
        }
    }
}
