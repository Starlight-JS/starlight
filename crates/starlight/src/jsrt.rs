/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::cell::{GcPointer, WeakRef},
    vm::{
        arguments::Arguments, arguments::JsArguments, array::JsArray, array_buffer::JsArrayBuffer,
        array_storage::ArrayStorage, attributes::*, code_block::CodeBlock,
        environment::Environment, error::*, function::*, global::JsGlobal,
        indexed_elements::IndexedElements, interpreter::SpreadValue, number::*, object::*,
        property_descriptor::*, string::*, structure::*, structure_chain::StructureChain,
        symbol_table::*, value::*, ModuleKind, Runtime,
    },
};
use std::collections::HashMap;
pub mod array;
pub mod array_buffer;
pub mod error;
#[cfg(all(target_pointer_width = "64", feature = "ffi"))]
pub mod ffi;
pub mod function;
pub mod generator;
pub mod global;
pub mod jsstd;
pub mod math;
pub mod number;
pub mod object;
pub mod regexp;
pub mod string;
pub mod symbol;
use array::*;
use error::*;
use function::*;
use wtf_rs::keep_on_stack;

pub fn print(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    for i in 0..args.size() {
        let value = args.at(i);
        let string = value.to_string(rt)?;
        print!("{}", string);
    }
    println!();
    Ok(JsValue::new(args.size() as i32))
}

impl Runtime {
    pub(crate) fn init_builtin(&mut self) {
        let _ = self.global_object().put(
            self,
            "Infinity".intern(),
            JsValue::new(std::f64::INFINITY),
            false,
        );
        let func = JsNativeFunction::new(self, "print".intern(), print, 0);
        self.global_object()
            .put(
                self,
                "print".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| unreachable!());

        string::initialize(self, self.global_data().object_prototype.unwrap());

        let mut global = self.global_object();
        let _ = global.put(
            self,
            "undefined".intern(),
            JsValue::encode_undefined_value(),
            false,
        );
        let func = JsNativeFunction::new(self, "isFinite".intern(), global::is_finite, 1);
        let _ = global.put(
            self,
            "isFinite".intern(),
            JsValue::encode_object_value(func),
            false,
        );

        let func = JsNativeFunction::new(self, "isNaN".intern(), global::is_nan, 1);
        let _ = global.put(
            self,
            "isNaN".intern(),
            JsValue::encode_object_value(func),
            false,
        );

        let func = JsNativeFunction::new(self, "parseInt".intern(), global::parse_int, 1);
        let _ = global.put(
            self,
            "parseInt".intern(),
            JsValue::encode_object_value(func),
            false,
        );
        let func = JsNativeFunction::new(self, "readLine".intern(), global::read_line, 1);
        let _ = global.put(
            self,
            "readLine".intern(),
            JsValue::encode_object_value(func),
            false,
        );

        let func = JsNativeFunction::new(self, "parseFloat".intern(), global::parse_float, 1);
        let _ = global.put(
            self,
            "parseFloat".intern(),
            JsValue::encode_object_value(func),
            false,
        );

        let func = JsNativeFunction::new(self, "gc".intern(), global::gc, 1);
        let _ = global.put(
            self,
            "gc".intern(),
            JsValue::encode_object_value(func),
            false,
        );
        let func = JsNativeFunction::new(self, "___trunc".intern(), global::___trunc, 1);
        let _ = global.put(
            self,
            "___trunc".intern(),
            JsValue::encode_object_value(func),
            false,
        );

        let func = JsNativeFunction::new(self, "___isCallable".intern(), global::___is_callable, 1);
        let _ = global.put(
            self,
            "___isCallable".intern(),
            JsValue::encode_object_value(func),
            false,
        );
        let func = JsNativeFunction::new(
            self,
            "___isConstructor".intern(),
            global::___is_constructor,
            1,
        );
        let _ = global.put(
            self,
            "___isConstructor".intern(),
            JsValue::encode_object_value(func),
            false,
        );
        let func = JsNativeFunction::new(self, "toString".intern(), global::to_string, 1);
        let _ = global.put(
            self,
            "toString".intern(),
            JsValue::encode_object_value(func),
            false,
        );
    }
    pub(crate) fn init_self_hosted(&mut self) {
        let mut eval = |path, source| {
            self.eval_internal(Some(path), false, source, true)
                .unwrap_or_else(|error| match error.to_string(self) {
                    Ok(str) => panic!("Failed to initialize builtins: {}", str),
                    Err(_) => panic!("Failed to initialize builtins"),
                });
        };

        eval(
            "builtins/GlobalOperations.js",
            include_str!("builtins/GlobalOperations.js"),
        );
        eval(
            "builtins/ArrayPrototype.js",
            include_str!("builtins/ArrayPrototype.js"),
        );
        eval(
            "builtins/StringPrototype.js",
            include_str!("builtins/StringPrototype.js"),
        );
        eval(
            "builtins/RegExpStringIterator.js",
            include_str!("builtins/RegExpStringIterator.js"),
        );
        eval(
            "builtins/RegExpPrototype.js",
            include_str!("builtins/RegExpPrototype.js"),
        );
        eval(
            "builtins/ArrayIterator.js",
            include_str!("builtins/ArrayIterator.js"),
        );
        eval(
            "builtins/StringIterator.js",
            include_str!("builtins/StringIterator.js"),
        );
    }
    pub(crate) fn init_func(&mut self, obj_proto: GcPointer<JsObject>) {
        let _structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let name = "Function".intern();

        let mut func_proto =
            JsNativeFunction::new_with_struct(self, &_structure, name, function_prototype, 1);
        self.global_data
            .function_struct
            .unwrap()
            .change_prototype_with_no_transition(func_proto);
        self.global_data.func_prototype = Some(func_proto);
        let func_ctor = JsNativeFunction::new(self, name, function_prototype, 1);

        let _ = self
            .global_object()
            .put(self, name, JsValue::from(func_ctor), false);
        let s = func_proto
            .structure()
            .change_prototype_transition(self, Some(obj_proto));
        (*func_proto).structure = s;

        let _ = func_proto.define_own_property(
            self,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::from(func_ctor), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, "bind".intern(), function_bind, 0);
        let name = "bind".intern();
        let _ = func_proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(f), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, "apply".intern(), function_apply, 0);
        let name = "apply".intern();
        let _ = func_proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(f), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, "call".intern(), function_call, 0);
        let name = "call".intern();
        let _ = func_proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(f), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, "toString".intern(), function_to_string, 0);
        let _ = func_proto.define_own_property(
            self,
            "toString".intern(),
            &*DataDescriptor::new(JsValue::from(f), W | C),
            false,
        );
    }
    pub(crate) fn init_array(&mut self, obj_proto: GcPointer<JsObject>) {
        let structure = Structure::new_indexed(self, None, true);
        self.global_data.array_structure = Some(structure);
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        self.global_data
            .array_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);
        let mut constructor = JsNativeFunction::new(self, "constructor".intern(), array_ctor, 1);

        let name = "Array".intern();
        let _ = self
            .global_object()
            .put(self, name, JsValue::from(constructor), false);

        let _ = constructor.define_own_property(
            self,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::from(proto), NONE),
            false,
        );

        let name = "isArray".intern();
        let is_array = JsNativeFunction::new(self, name, array_is_array, 1);
        let _ = constructor.put(self, name, JsValue::from(is_array), false);
        let name = "of".intern();
        let array_of = JsNativeFunction::new(self, name, array_of, 1);
        let _ = constructor.put(self, name, JsValue::from(array_of), false);
        let name = "from".intern();
        let array_from = JsNativeFunction::new(self, name, array_from, 1);
        let _ = constructor.put(self, name, JsValue::from(array_from), false);
        let _ = proto.define_own_property(
            self,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::from(constructor), W | C),
            false,
        );
        let name = "join".intern();
        let join = JsNativeFunction::new(self, name, array_join, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(join), W | C | E),
            false,
        );

        let name = "toString".intern();
        let to_string = JsNativeFunction::new(self, name, array_join, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(to_string), W | C | E),
            false,
        );

        let name = "push".intern();
        let push = JsNativeFunction::new(self, name, array_push, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(push), W | C | E),
            false,
        );
        let name = "pop".intern();
        let pop = JsNativeFunction::new(self, name, array_pop, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(pop), W | C | E),
            false,
        );
        let name = "reduce".intern();
        let reduce = JsNativeFunction::new(self, name, array_reduce, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(reduce), W | C | E),
            false,
        );

        let name = "slice".intern();
        let slice = JsNativeFunction::new(self, name, array_slice, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(slice), W | C | E),
            false,
        );
        let _ = { def_native_method!(self, proto, shift, array::array_shift, 0) };
        /*let name = "forEach".intern();
        let for_each = JsNativeFunction::new(self, name, array_for_each, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(for_each), W | C | E),
            false,
        );*/

        /*let name = "filter".intern();
        let filter = JsNativeFunction::new(self, name, array_filter, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(filter), W | C | E),
            false,
        );*/

        /*let name = "map".intern();
        let map = JsNativeFunction::new(self, name, array_map, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(map), W | C | E),
            false,
        );*/
        let name = "concat".intern();
        let concat = JsNativeFunction::new(self, name, array_concat, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::from(concat), W | C | E),
            false,
        );
        self.global_data.array_prototype = Some(proto);
        let arr = "Array".intern();
        let _ = self.global_object().define_own_property(
            self,
            arr,
            &*DataDescriptor::new(JsValue::from(constructor), W | C),
            false,
        );
    }
    pub(crate) fn init_error(&mut self, obj_proto: GcPointer<JsObject>) {
        self.global_data.error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.eval_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.range_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.reference_error_structure =
            Some(Structure::new_indexed(self, None, false));
        self.global_data.type_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.syntax_error_structure = Some(Structure::new_indexed(self, None, false));
        let structure = Structure::new_unique_with_proto(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsError::get_class(), ObjectTag::Ordinary);
        self.global_data.error = Some(proto);
        let e = "Error".intern();
        let mut ctor = JsNativeFunction::new(self, e, error_constructor, 1);
        let _ = ctor.define_own_property(
            self,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::from(proto), NONE),
            false,
        );
        proto.class = JsError::get_class();
        let _ = proto.define_own_property(
            self,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::from(ctor), W | C),
            false,
        );

        let n = "name".intern();
        let s = JsString::new(self, "Error");
        let e = JsString::new(self, "");
        let m = "message".intern();
        let _ = proto.define_own_property(
            self,
            n,
            &*DataDescriptor::new(JsValue::from(s), W | C),
            false,
        );

        let _ = proto.define_own_property(
            self,
            m,
            &*DataDescriptor::new(JsValue::from(e), W | C),
            false,
        );
        let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
        let _ = proto.define_own_property(
            self,
            "toString".intern(),
            &*DataDescriptor::new(JsValue::from(to_str), W | C),
            false,
        );
        let sym = "Error".intern();
        let _ = self.global_object().define_own_property(
            self,
            sym,
            &*DataDescriptor::new(JsValue::from(ctor), W | C),
            false,
        );

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsEvalError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .eval_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = "EvalError".intern();
            let mut sub_ctor = JsNativeFunction::new(self, sym, eval_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::from(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            let n = "name".intern();
            let s = JsString::new(self, "EvalError");
            let e = JsString::new(self, "");
            let m = "message".intern();
            let _ = sub_proto.define_own_property(
                self,
                n,
                &*DataDescriptor::new(JsValue::from(s), W | C),
                false,
            );

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::from(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
            let _ = sub_proto.define_own_property(
                self,
                "toString".intern(),
                &*DataDescriptor::new(JsValue::from(to_str), W | C),
                false,
            );
            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            self.global_data.eval_error = Some(sub_proto);
        }

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsTypeError::get_class(),
                ObjectTag::Ordinary,
            );

            keep_on_stack!(&structure, &mut sub_proto);

            self.global_data
                .type_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = "TypeError".intern();
            let mut sub_ctor = JsNativeFunction::new(self, sym, type_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::from(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            let n = "name".intern();
            let s = JsString::new(self, "TypeError");
            let e = JsString::new(self, "");
            let m = "message".intern();
            let _ = sub_proto
                .define_own_property(
                    self,
                    n,
                    &*DataDescriptor::new(JsValue::from(s), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::from(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
            let _ = sub_proto
                .define_own_property(
                    self,
                    "toString".intern(),
                    &*DataDescriptor::new(JsValue::from(to_str), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());
            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            self.global_data.type_error = Some(sub_proto);
        }
        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsSyntaxError::get_class(),
                ObjectTag::Ordinary,
            );

            keep_on_stack!(&structure, &mut sub_proto);

            self.global_data
                .syntax_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = "SyntaxError".intern();
            let mut sub_ctor = JsNativeFunction::new(self, sym, syntax_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::from(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            let n = "name".intern();
            let s = JsString::new(self, "SyntaxError");
            let e = JsString::new(self, "");
            let m = "message".intern();
            let _ = sub_proto
                .define_own_property(
                    self,
                    n,
                    &*DataDescriptor::new(JsValue::from(s), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::from(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
            let _ = sub_proto
                .define_own_property(
                    self,
                    "toString".intern(),
                    &*DataDescriptor::new(JsValue::from(to_str), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());
            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            self.global_data.syntax_error = Some(sub_proto);
        }

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsReferenceError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .reference_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = "ReferenceError".intern();
            let mut sub_ctor = JsNativeFunction::new(self, sym, reference_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::from(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            let n = "name".intern();
            let s = JsString::new(self, "ReferenceError");
            let e = JsString::new(self, "");
            let m = "message".intern();
            let _ = sub_proto.define_own_property(
                self,
                n,
                &*DataDescriptor::new(JsValue::from(s), W | C),
                false,
            );

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::from(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
            let _ = sub_proto.define_own_property(
                self,
                "toString".intern(),
                &*DataDescriptor::new(JsValue::from(to_str), W | C),
                false,
            );

            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::from(sub_proto), W | C),
                false,
            );

            self.global_data.reference_error = Some(sub_proto);
        }

        // range error
        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsReferenceError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .range_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = "RangeError".intern();
            let mut sub_ctor = JsNativeFunction::new(self, sym, range_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::from(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::from(sub_ctor), W | C),
                false,
            );

            let n = "name".intern();
            let s = JsString::new(self, "RangeError");
            let e = JsString::new(self, "");
            let m = "message".intern();
            let _ = sub_proto.define_own_property(
                self,
                n,
                &*DataDescriptor::new(JsValue::from(s), W | C),
                false,
            );

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::from(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, "toString".intern(), error_to_string, 0);
            let _ = sub_proto.define_own_property(
                self,
                "toString".intern(),
                &*DataDescriptor::new(JsValue::from(to_str), W | C),
                false,
            );

            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::from(sub_proto), W | C),
                false,
            );

            self.global_data.range_error = Some(sub_proto);
        }
    }
}

use object::*;

pub(crate) fn object_init(
    rt: &mut Runtime,
    mut obj_constructor: GcPointer<JsObject>,
    mut proto: GcPointer<JsObject>,
) {
    let func = JsNativeFunction::new(rt, "defineProperty".intern(), object_define_property, 3);
    let _ = obj_constructor.define_own_property(
        rt,
        "defineProperty".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "seal".intern(), object_seal, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "seal".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "freeze".intern(), object_freeze, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "freeze".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "isSealed".intern(), object_is_sealed, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "isSealed".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "isFrozen".intern(), object_is_frozen, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "isFrozen".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "isExtensible".intern(), object_is_extensible, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "isExtensible".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );
    let func = JsNativeFunction::new(rt, "getPrototypeOf".intern(), object_get_prototype_of, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "getPrototypeOf".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );
    let func = JsNativeFunction::new(
        rt,
        "preventExtensions".intern(),
        object_prevent_extensions,
        1,
    );
    let _ = obj_constructor.define_own_property(
        rt,
        "preventExtensions".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let func = JsNativeFunction::new(rt, "keys".intern(), object_keys, 1);
    let _ = obj_constructor.define_own_property(
        rt,
        "keys".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );
    let func = JsNativeFunction::new(
        rt,
        "getOwnPropertyDescriptor".intern(),
        object_get_own_property_descriptor,
        2,
    );
    let _ = obj_constructor.define_own_property(
        rt,
        "getOwnPropertyDescriptor".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );
    let func = JsNativeFunction::new(rt, "create".intern(), object_create, 3);
    let _ = obj_constructor.define_own_property(
        rt,
        "create".intern(),
        &*DataDescriptor::new(JsValue::new(func), NONE),
        false,
    );

    let _ = obj_constructor.define_own_property(
        rt,
        "prototype".intern(),
        &*DataDescriptor::new(JsValue::from(proto), NONE),
        false,
    );
    let _ = proto.define_own_property(
        rt,
        "constructor".intern(),
        &*DataDescriptor::new(JsValue::from(obj_constructor), W | C),
        false,
    );
    let obj_to_string = JsNativeFunction::new(rt, "toString".intern(), object_to_string, 0);
    let _ = proto.define_own_property(
        rt,
        "toString".intern(),
        &*DataDescriptor::new(JsValue::from(obj_to_string), W | C),
        false,
    );

    let func = JsNativeFunction::new(rt, "hasOwnProperty".intern(), has_own_property, 1);
    let _ = proto.define_own_property(
        rt,
        "hasOwnProperty".intern(),
        &*DataDescriptor::new(JsValue::from(func), W | C),
        false,
    );
}
use crate::gc::snapshot::deserializer::*;
use once_cell::sync::Lazy;

pub static VM_NATIVE_REFERENCES: Lazy<&'static [usize]> = Lazy::new(|| {
    let mut refs = vec![
        /* deserializer functions */
        // following GcPointer and WeakRef method references is obtained from `T = u8`
        // but they should be the same for all types that is allocated in GC heap.
        Vec::<crate::gc::cell::GcPointer<crate::vm::structure::Structure>>::deserialize as _,
        Vec::<crate::gc::cell::GcPointer<crate::vm::structure::Structure>>::allocate as _,
        GcPointer::<u8>::deserialize as _,
        GcPointer::<u8>::allocate as _,
        WeakRef::<u8>::deserialize as _,
        WeakRef::<u8>::allocate as _,
        JsObject::deserialize as _,
        JsObject::allocate as _,
        JsValue::deserialize as _,
        JsValue::allocate as _,
        TargetTable::deserialize as _,
        TargetTable::allocate as _,
        SpreadValue::deserialize as _,
        SpreadValue::allocate as _,
        Structure::deserialize as _,
        Structure::allocate as _,
        crate::vm::structure::Table::deserialize as _,
        crate::vm::structure::Table::allocate as _,
        ArrayStorage::deserialize as _,
        ArrayStorage::allocate as _,
        DeletedEntry::deserialize as _,
        DeletedEntry::allocate as _,
        JsString::deserialize as _,
        JsString::allocate as _,
        u8::deserialize as _,
        u8::allocate as _,
        u16::deserialize as _,
        u16::allocate as _,
        u32::deserialize as _,
        u32::allocate as _,
        u64::deserialize as _,
        u64::allocate as _,
        i8::deserialize as _,
        i8::allocate as _,
        i16::deserialize as _,
        i16::allocate as _,
        i32::deserialize as _,
        i32::allocate as _,
        i64::deserialize as _,
        i64::allocate as _,
        HashMap::<u32, StoredSlot>::deserialize as _,
        HashMap::<u32, StoredSlot>::allocate as _,
        IndexedElements::deserialize as _,
        IndexedElements::allocate as _,
        CodeBlock::deserialize as _,
        CodeBlock::allocate as _,
        JsArguments::get_class() as *const _ as usize,
        JsObject::get_class() as *const _ as usize,
        JsArray::get_class() as *const _ as usize,
        JsFunction::get_class() as *const _ as usize,
        JsError::get_class() as *const _ as usize,
        JsTypeError::get_class() as *const _ as usize,
        JsSyntaxError::get_class() as *const _ as usize,
        JsReferenceError::get_class() as *const _ as usize,
        JsRangeError::get_class() as *const _ as usize,
        JsEvalError::get_class() as *const _ as usize,
        JsGlobal::get_class() as *const _ as usize,
        function::function_bind as usize,
        function::function_prototype as usize,
        function::function_to_string as usize,
        function::function_apply as usize,
        function::function_call as usize,
        object::object_constructor as usize,
        object::object_create as usize,
        object::object_to_string as usize,
        object::object_define_property as usize,
        object::has_own_property as usize,
        object::object_keys as usize,
        object::object_get_own_property_descriptor as usize,
        object::object_freeze as _,
        object::object_seal as _,
        object::object_get_prototype_of as _,
        object::object_is_extensible as _,
        object::object_is_sealed as _,
        object::object_is_frozen as _,
        object::object_prevent_extensions as _,
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
        error::error_constructor as usize,
        error::error_to_string as usize,
        error::eval_error_constructor as usize,
        error::range_error_constructor as usize,
        error::reference_error_constructor as usize,
        error::syntax_error_constructor as usize,
        error::type_error_constructor as usize,
        print as usize,
        global::is_finite as _,
        global::is_nan as _,
        global::parse_float as _,
        global::parse_int as _,
        global::read_line as _,
        global::gc as _,
        global::___is_constructor as _,
        global::___is_callable as _,
        global::___trunc as _,
        global::to_string as _,
        string::string_concat as _,
        string::string_trim as _,
        string::string_trim_start as _,
        string::string_trim_end as _,
        string::string_pad_start as _,
        string::string_pad_end as _,
        string::string_split as _,
        string::string_constructor as _,
        string::string_to_string as _,
        string::string_index_of as _,
        string::string_last_index_of as _,
        string::string_substr as _,
        string::string_substring as _,
        string::string_replace as _,
        string::string_value_of as _,
        string::string_char_at as _,
        string::string_char_code_at as _,
        string::string_code_point_at as _,
        string::string_starts_with as _,
        string::string_ends_with as _,
        string::string_repeat as _,
        string::string_to_lowercase as _,
        string::string_to_uppercase as _,
        string::string_includes as _,
        string::string_slice as _,
        JsStringObject::get_class() as *const _ as usize,
        NumberObject::get_class() as *const _ as usize,
        Environment::deserialize as _,
        Environment::allocate as _,
        number::number_constructor as _,
        number::number_clz as _,
        number::number_is_finite as _,
        number::number_is_integer as _,
        number::number_is_nan as _,
        number::number_to_int as _,
        number::number_to_precisiion as _,
        number::number_to_fixed as _,
        number::number_to_string as _,
        number::number_value_of as _,
        math::math_trunc as _,
        math::math_floor as _,
        math::math_log as _,
        math::math_sin as _,
        math::math_cos as _,
        math::math_ceil as _,
        math::math_exp as _,
        math::math_abs as _,
        math::math_sqrt as _,
        math::math_random as _,
        StructureChain::deserialize as _,
        StructureChain::allocate as _,
        HashValueZero::deserialize as _,
        HashValueZero::allocate as _,
        regexp::regexp_constructor as _,
        regexp::regexp_exec as _,
        regexp::regexp_test as _,
        regexp::regexp_to_string as _,
        regexp::regexp_match as _,
        regexp::regexp_split_fast as _,
        symbol::symbol_ctor as _,
        symbol::symbol_for as _,
        symbol::symbol_key_for as _,
        symbol::symbol_to_string as _,
        symbol::symbol_value_of as _,
        JsSymbol::deserialize as _,
        JsSymbol::allocate as _,
        Accessor::deserialize as _,
        Accessor::allocate as _,
        module_load as _,
        jsstd::init_js_std as _,
        jsstd::file::std_file_open as _,
        jsstd::file::std_file_read as _,
        jsstd::file::std_file_write as _,
        jsstd::file::std_file_write_all as _,
        jsstd::file::std_file_read_bytes as _,
        jsstd::file::std_file_read_bytes_exact as _,
        jsstd::file::std_file_read_bytes_to_end as _,
        jsstd::file::std_file_close as _,
        jsstd::std_args as _,
        generator::generator_next as _,
        generator::generator_iterator as _,
        generator::generator_return as _,
        generator::generator_throw as _,
        array_buffer::array_buffer_constructor as _,
        array_buffer::array_buffer_byte_length as _,
        array_buffer::array_buffer_slice as _,
        JsArrayBuffer::get_class() as *const _ as usize,
    ];
    #[cfg(all(target_pointer_width = "64", feature = "ffi"))]
    {
        refs.push(ffi::ffi_function_attach as _);
        refs.push(ffi::ffi_function_call as _);
        refs.push(ffi::ffi_library_open as _);
    }
    // refs.sort_unstable();
    // refs.dedup();
    Box::leak(refs.into_boxed_slice())
});

pub fn get_length(rt: &mut Runtime, val: &mut GcPointer<JsObject>) -> Result<u32, JsValue> {
    if std::ptr::eq(val.class(), JsArray::get_class()) {
        return Ok(val.indexed.length());
    }
    let len = val.get(rt, "length".intern())?;
    len.to_uint32(rt)
}

/// Convert JS object to JS property descriptor
pub fn to_property_descriptor(
    rt: &mut Runtime,
    target: JsValue,
) -> Result<PropertyDescriptor, JsValue> {
    if !target.is_jsobject() {
        return Err(JsValue::new(
            rt.new_type_error("ToPropertyDescriptor requires Object argument"),
        ));
    }

    let mut attr: u32 = DEFAULT;
    let stack = rt.shadowstack();
    letroot!(obj = stack, target.get_jsobject());
    let mut value = JsValue::encode_undefined_value();
    let mut getter = JsValue::encode_undefined_value();
    let mut setter = JsValue::encode_undefined_value();

    {
        let sym = "enumerable".intern();
        if obj.has_property(rt, sym) {
            let enumerable = obj.get(rt, sym)?.to_boolean();
            if enumerable {
                attr = (attr & !UNDEF_ENUMERABLE) | ENUMERABLE;
            } else {
                attr &= !UNDEF_ENUMERABLE;
            }
        }
    }
    {
        let sym = "configurable".intern();
        if obj.has_property(rt, sym) {
            let configurable = obj.get(rt, sym)?.to_boolean();
            if configurable {
                attr = (attr & !UNDEF_CONFIGURABLE) | CONFIGURABLE;
            } else {
                attr &= !UNDEF_CONFIGURABLE;
            }
        }
    }

    {
        let sym = "value".intern();
        if obj.has_property(rt, sym) {
            value = obj.get(rt, sym)?;
            attr |= DATA;
            attr &= !UNDEF_VALUE;
        }
    }

    {
        let sym = "writable".intern();
        if obj.has_property(rt, sym) {
            let writable = obj.get(rt, sym)?.to_boolean();
            attr |= DATA;
            attr &= !UNDEF_WRITABLE;
            if writable {
                attr |= WRITABLE;
            }
        }
    }
    {
        let sym = "get".intern();
        if obj.has_property(rt, sym) {
            let r = obj.get(rt, sym)?;
            if !r.is_callable() && !r.is_undefined() {
                return Err(JsValue::new(
                    rt.new_type_error("property 'get' is not callable"),
                ));
            }

            attr |= ACCESSOR;
            if !r.is_undefined() {
                getter = r;
            }

            attr &= !UNDEF_GETTER;
        }
    }

    {
        let sym = "set".intern();
        if obj.has_property(rt, sym) {
            let r = obj.get(rt, sym)?;
            if !r.is_callable() && !r.is_undefined() {
                return Err(JsValue::new(
                    rt.new_type_error("property 'set' is not callable"),
                ));
            }

            attr |= ACCESSOR;
            if !r.is_undefined() {
                setter = r;
            }
            attr &= !UNDEF_SETTER;
        }
    }

    if (attr & ACCESSOR) != 0 && (attr & DATA) != 0 {
        return Err(JsValue::new(
            rt.new_type_error("invalid property descriptor object"),
        ));
    }

    if (attr & ACCESSOR) != 0 {
        attr &= !DATA;
        return Ok(*AccessorDescriptor::new(getter, setter, attr));
    } else if (attr & DATA) != 0 {
        return Ok(*DataDescriptor::new(value, attr));
    } else {
        return Ok(*GenericDescriptor::new(attr));
    }
}

pub(crate) fn module_load(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let name = args.at(0).to_string(rt)?;
    let rel_path = unsafe { (*rt.stack.current).code_block.unwrap().path.clone() };
    let is_js_load = (name.starts_with("./")
        || name.starts_with("../")
        || name.starts_with('/')
        || name.ends_with(".js"))
        && name.ends_with(".js");
    let spath = name;
    let mut spath = if rel_path.is_empty() {
        spath
    } else {
        format!("{}/{}", rel_path, spath)
    };
    if cfg!(windows) {
        spath = spath.replace("/", "\\");
    }
    let path = std::path::Path::new(&spath);
    let path = match path.canonicalize() {
        Err(e) => {
            return Err(JsValue::new(rt.new_reference_error(format!(
                "Module '{}' not found: '{}'",
                path.display(),
                e
            ))))
        }
        Ok(path) => path,
    };
    let stack = rt.shadowstack();
    letroot!(module_object = stack, JsObject::new_empty(rt));
    let mut exports = JsObject::new_empty(rt);
    module_object.put(rt, "@exports".intern(), JsValue::new(exports), false)?;
    let mut args = [JsValue::new(*module_object)];
    letroot!(
        args = stack,
        Arguments::new(JsValue::encode_undefined_value(), &mut args)
    );
    if let Some(module) = rt.modules.get(&spath).copied() {
        match module {
            ModuleKind::Initialized(x) => {
                return Ok(JsValue::new(x));
            }
            ModuleKind::NativeUninit(init) => {
                let mut module = *module_object;
                init(rt, module)?;
                rt.modules
                    .insert(spath.clone(), ModuleKind::Initialized(module));

                return Ok(JsValue::new(module));
            }
        }
    }
    if !path.exists() {
        return Err(JsValue::new(
            rt.new_type_error(format!("Module '{}' not found", spath)),
        ));
    }
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(e) => {
            return Err(JsValue::new(rt.new_type_error(format!(
                "Failed to read module '{}': {}",
                spath,
                e.to_string()
            ))));
        }
    };
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    let module_fun = rt.compile_module(&spath, &name, &source)?;
    let mut module_fun = module_fun.get_jsobject();
    module_fun
        .as_function_mut()
        .call(rt, &mut args, JsValue::encode_undefined_value())?;
    rt.modules
        .insert(spath.clone(), ModuleKind::Initialized(*module_object));
    Ok(JsValue::new(*module_object))
}
