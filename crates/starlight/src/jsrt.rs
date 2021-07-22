/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    constant::*,
    gc::cell::{GcPointer, WeakRef, WeakSlot},
    vm::{
        arguments::Arguments, arguments::JsArguments, array::JsArray, array_buffer::JsArrayBuffer,
        array_storage::ArrayStorage, attributes::*, code_block::CodeBlock, context::Context,
        data_view::JsDataView, environment::Environment, error::*, function::*, global::JsGlobal,
        indexed_elements::IndexedElements, interpreter::SpreadValue, number::*, object::*,
        property_descriptor::*, string::*, structure::*, structure_chain::StructureChain,
        symbol_table::*, value::*, ModuleKind,
    },
};
use std::{collections::HashMap, rc::Rc};
pub mod array;
pub mod array_buffer;
pub mod boolean;
pub mod data_view;
pub mod date;
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
pub mod promise;
pub mod regexp;
pub mod string;
pub mod symbol;
pub mod weak_ref;
use array::*;
use error::*;
use function::*;
use promise::*;
use wtf_rs::keep_on_stack;

pub fn print(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    for i in 0..args.size() {
        let value = args.at(i);
        let string = value.to_string(ctx)?;
        print!("{}", string);
    }
    println!();
    Ok(JsValue::new(args.size() as i32))
}

impl GcPointer<Context> {
    pub(crate) fn init_builtin_in_global_object(mut self) -> Result<(), JsValue> {
        let mut global_object = self.global_object();

        // Property
        def_native_property!(self, global_object, undefined, JsValue::UNDEFINED)?;

        let nan = JsValue::encode_nan_value();
        def_native_property!(self, global_object, NaN, nan)?;
        def_native_property!(self, global_object, Infinity, std::f64::INFINITY)?;

        // Method
        def_native_method!(self, global_object, print, print, 0)?;
        def_native_method!(self, global_object, isFinite, global::is_finite, 1)?;
        def_native_method!(self, global_object, isNaN, global::is_nan, 1)?;
        def_native_method!(self, global_object, parseInt, global::parse_int, 1)?;
        def_native_method!(self, global_object, readLine, global::read_line, 1)?;
        def_native_method!(self, global_object, parseFloat, global::parse_float, 1)?;
        def_native_method!(self, global_object, gc, global::gc, 0)?;
        def_native_method!(self, global_object, ___trunc, global::___trunc, 1)?;
        def_native_method!(
            self,
            global_object,
            ___isCallable,
            global::___is_callable,
            1
        )?;
        def_native_method!(
            self,
            global_object,
            ___isConstructor,
            global::___is_constructor,
            1
        )?;
        def_native_method!(self, global_object, toString, global::to_string, 1)?;

        Ok(())
    }
    pub(crate) fn init_self_hosted(mut self) {
        let spread = include_str!("builtins/Spread.js");
        let func = self
            .compile_function("@spread", spread, &["iterable".to_string()])
            .unwrap_or_else(|_| panic!());
        assert!(func.is_callable());
        self.global_data.spread_builtin = Some(func.get_jsobject());

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
        eval("builtins/Object.js", include_str!("builtins/Object.js"))
    }
    pub(crate) fn init_func_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.func_prototype.unwrap();
        let name = S_FUNCTION.intern();
        let constrcutor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();
        let _ = self.global_object().put(self, name, constrcutor, false);
        Ok(())
    }
    pub(crate) fn init_func_global_data(
        mut self,
        obj_proto: GcPointer<JsObject>,
    ) -> Result<(), JsValue> {
        let _structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let name = S_FUNCTION.intern();

        let mut func_proto =
            JsNativeFunction::new_with_struct(self, &_structure, name, function_prototype, 1);
        self.global_data
            .function_struct
            .unwrap()
            .change_prototype_with_no_transition(func_proto);
        self.global_data.func_prototype = Some(func_proto);
        let s = func_proto
            .structure()
            .change_prototype_transition(self, Some(obj_proto));
        (*func_proto).structure = s;
        let mut func_ctor = JsNativeFunction::new(self, name, function_prototype, 1);

        def_native_property!(self, func_ctor, prototype, func_proto, NONE)?;
        def_native_property!(self, func_proto, constructor, func_ctor, W | C)?;

        def_native_method!(self, func_proto, bind, function_bind, 0, W | C)?;
        def_native_method!(self, func_proto, apply, function_apply, 0, W | C)?;
        def_native_method!(self, func_proto, call, function_call, 0, W | C)?;
        def_native_method!(self, func_proto, toString, function_to_string, 0, W | C)?;
        Ok(())
    }
    pub(crate) fn init_promise_in_global_object(mut self) -> Result<(), JsValue> {
        // copied from file
        let mut ctor = JsNativeFunction::new(self, S_PROMISE.intern(), promise_constructor, 1);
        let mut global_object = self.global_object();
        let mut proto = JsObject::new_empty(self);

        // members / proto
        def_native_method!(self, proto, then, promise_then, 2)?;
        def_native_method!(self, proto, catch, promise_catch, 1)?;
        def_native_method!(self, proto, finally, promise_finally, 1)?;
        def_native_method!(self, proto, resolve, promise_resolve, 1)?;
        def_native_method!(self, proto, reject, promise_reject, 1)?;
        // statics
        def_native_method!(self, ctor, all, promise_static_all, 1)?;
        def_native_method!(self, ctor, allSettled, promise_static_all_settled, 1)?;
        def_native_method!(self, ctor, any, promise_static_any, 1)?;
        def_native_method!(self, ctor, race, promise_static_race, 1)?;
        def_native_method!(self, ctor, reject, promise_static_reject, 1)?;
        def_native_method!(self, ctor, resolve, promise_static_resolve, 1)?;

        def_native_property!(self, ctor, prototype, proto)?;
        def_native_property!(self, proto, constructor, ctor)?;
        def_native_property!(self, global_object, Promise, ctor)?;

        Ok(())
    }
    pub(crate) fn init_weak_ref_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data().weak_ref_prototype.unwrap();
        let ctor = proto.get(self, S_CONSTURCTOR.intern())?;
        let mut global_object = self.global_object();

        def_native_property!(self, global_object, WeakRef, ctor)?;
        Ok(())
    }
    pub(crate) fn init_weak_ref_in_global_data(mut self) -> Result<(), JsValue> {
        let obj_proto = self.global_data().object_prototype.unwrap();
        self.global_data.weak_ref_structure = Some(Structure::new_indexed(self, None, false));
        let proto_map = self
            .global_data
            .weak_ref_structure
            .unwrap()
            .change_prototype_transition(self, Some(obj_proto));
        let mut proto = JsObject::new(self, &proto_map, JsObject::get_class(), ObjectTag::Ordinary);
        self.global_data
            .weak_ref_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);

        let mut ctor =
            JsNativeFunction::new(self, S_WEAK_REF.intern(), weak_ref::weak_ref_constructor, 1);

        def_native_property!(self, proto, constructor, ctor)?;
        def_native_property!(self, ctor, prototype, proto)?;

        def_native_method!(self, proto, deref, weak_ref::weak_ref_prototype_deref, 0)?;

        self.global_data.weak_ref_prototype = Some(proto);
        Ok(())
    }

    pub(crate) fn init_array_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.array_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();
        let mut global_object = self.global_object();

        def_native_property!(self, global_object, Array, constructor, W | C)?;

        Ok(())
    }

    pub(crate) fn init_array_in_global_data(
        mut self,
        obj_proto: GcPointer<JsObject>,
    ) -> Result<(), JsValue> {
        let structure = Structure::new_indexed(self, None, true);
        self.global_data.array_structure = Some(structure);
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        self.global_data
            .array_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);
        let mut constructor = JsNativeFunction::new(self, S_CONSTURCTOR.intern(), array_ctor, 1);

        def_native_property!(self, constructor, prototype, proto, NONE)?;
        def_native_method!(self, constructor, isArray, array_is_array, 1)?;
        def_native_method!(self, constructor, of, array_of, 1)?;
        def_native_method!(self, constructor, from, array_from, 1)?;
        def_native_property!(self, proto, constructor, constructor, W | C)?;
        def_native_method!(self, proto, join, array_join, 1, W | C | E)?;
        def_native_method!(self, proto, toString, array_to_string, 1, W | C | E)?;
        def_native_method!(self, proto, push, array_push, 1, W | C | E)?;
        def_native_method!(self, proto, pop, array_pop, 1, W | C | E)?;
        def_native_method!(self, proto, reduce, array_reduce, 1, W | C | E)?;
        def_native_method!(self, proto, slice, array_slice, 1, W | C | E)?;
        def_native_method!(self, proto, shift, array::array_shift, 0)?;
        def_native_method!(self, proto, concat, array_concat, 1, W | C | E)?;
        self.global_data.array_prototype = Some(proto);

        Ok(())
    }

    pub(crate) fn init_error_in_global_object(mut self) -> Result<(), JsValue> {
        self.init_base_error_in_global_object()?;
        self.init_eval_error_in_global_object()?;
        self.init_type_error_in_global_object()?;
        self.init_syntax_error_in_global_object()?;
        self.init_reference_error_in_global_object()?;
        self.init_range_error_in_global_object()?;
        self.init_uri_error_in_global_object()?;
        Ok(())
    }

    pub(crate) fn init_base_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.error.unwrap();
        let e = S_ERROR.intern();
        let mut ctor = JsNativeFunction::new(self, e, error_constructor, 1);
        proto.class = JsError::get_class();

        let s = JsString::new(self, S_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, ctor, prototype, proto, NONE)?;
        def_native_property!(self, proto, constructor, ctor, W | C)?;
        def_native_property!(self, proto, name, s, W | C)?;

        def_native_property!(self, proto, message, e, W | C)?;
        def_native_method!(self, proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, Error, ctor, W | C)?;

        Ok(())
    }

    pub(crate) fn init_eval_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.eval_error.unwrap();
        let sym = S_EVAL_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, eval_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_EVAL_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;
        def_native_property!(self, sub_proto, message, e, W | C)?;

        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, EvalError, sub_ctor, W | C)?;

        Ok(())
    }

    pub(crate) fn init_type_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.type_error.unwrap();
        let sym = S_TYPE_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, type_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_TYPE_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;
        def_native_property!(self, sub_proto, message, e, W | C)?;
        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, TypeError, sub_ctor, W | C)?;
        Ok(())
    }

    pub(crate) fn init_syntax_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.syntax_error.unwrap();
        let sym = S_SYNTAX_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, syntax_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_SYNTAX_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;

        def_native_property!(self, sub_proto, message, e, W | C)?;

        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, SyntaxError, sub_ctor, W | C)?;

        Ok(())
    }

    pub(crate) fn init_reference_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.reference_error.unwrap();
        let sym = S_REFERENCE_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, reference_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_REFERENCE_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;
        def_native_property!(self, sub_proto, message, e, W | C)?;

        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, ReferenceError, sub_ctor, W | C)?;

        Ok(())
    }

    pub(crate) fn init_range_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.range_error.unwrap();
        let sym = S_RANGE_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, range_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_RANGE_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;
        def_native_property!(self, sub_proto, message, e, W | C)?;
        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, RangeError, sub_proto, W | C)?;
        Ok(())
    }

    pub(crate) fn init_uri_error_in_global_object(mut self) -> Result<(), JsValue> {
        let mut sub_proto = self.global_data.uri_error.unwrap();
        let sym = S_URI_ERROR.intern();
        let mut sub_ctor = JsNativeFunction::new(self, sym, uri_error_constructor, 1);

        def_native_property!(self, sub_ctor, prototype, sub_proto, NONE)?;
        def_native_property!(self, sub_proto, constructor, sub_ctor, W | C)?;

        let s = JsString::new(self, S_URI_ERROR);
        let e = JsString::new(self, "");

        def_native_property!(self, sub_proto, name, s, C)?;
        def_native_property!(self, sub_proto, message, e, W | C)?;
        def_native_method!(self, sub_proto, toString, error_to_string, 0, W | C)?;

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, URIError, sub_proto, W | C)?;
        Ok(())
    }

    pub(crate) fn init_error_in_global_data(
        mut self,
        obj_proto: GcPointer<JsObject>,
    ) -> Result<(), JsValue> {
        self.global_data.error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.eval_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.range_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.reference_error_structure =
            Some(Structure::new_indexed(self, None, false));
        self.global_data.type_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.syntax_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.uri_error_structure = Some(Structure::new_indexed(self, None, false));

        let structure = Structure::new_unique_with_proto(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, JsError::get_class(), ObjectTag::Ordinary);
        self.global_data.error = Some(proto);

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
            self.global_data.range_error = Some(sub_proto);
        }

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                &structure,
                JsURIError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .uri_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            self.global_data.uri_error = Some(sub_proto);
        }

        Ok(())
    }
}

use object::*;

impl GcPointer<Context> {
    pub(crate) fn init_object_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.object_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, Object, constructor, W | C)?;
        def_native_property!(self, global_object, globalThis, global_object)?;
        Ok(())
    }

    pub(crate) fn init_object_in_global_data(
        mut self,
        mut proto: GcPointer<JsObject>,
    ) -> Result<(), JsValue> {
        let name = S_OBJECT.intern();
        let mut ctor = JsNativeFunction::new(self, name, object_constructor, 1);
        self.global_data.object_constructor = Some(ctor);

        def_native_method!(self, ctor, defineProperty, object_define_property, 3, NONE)?;

        def_native_method!(self, ctor, seal, object_seal, 1, NONE)?;

        def_native_method!(self, ctor, freeze, object_freeze, 1, NONE)?;

        def_native_method!(self, ctor, isSealed, object_is_sealed, 1, NONE)?;

        def_native_method!(self, ctor, isFrozen, object_is_frozen, 1, NONE)?;

        def_native_method!(self, ctor, isExtensible, object_is_extensible, 1, NONE)?;

        def_native_method!(self, ctor, getPrototypeOf, object_get_prototype_of, 1, NONE)?;

        def_native_method!(
            self,
            ctor,
            preventExtensions,
            object_prevent_extensions,
            1,
            NONE
        )?;

        def_native_method!(self, ctor, keys, object_keys, 1, NONE)?;

        def_native_method!(
            self,
            ctor,
            getOwnPropertyDescriptor,
            object_get_own_property_descriptor,
            2,
            NONE
        )?;

        def_native_method!(self, ctor, create, object_create, 3, NONE)?;

        def_native_property!(self, ctor, prototype, proto, NONE)?;

        def_native_property!(self, proto, constructor, ctor, W | C)?;

        def_native_method!(self, proto, toString, object_to_string, 0, W | C)?;

        def_native_method!(
            self,
            proto,
            hasOwnProperty,
            object_has_own_property,
            1,
            W | C
        )?;

        def_native_method!(
            self,
            proto,
            propertyIsEnumerable,
            object_property_is_enumerable,
            1,
            W | C
        )?;
        Ok(())
    }
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
        Context::deserialize as _,
        Context::allocate as _,
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
        JsURIError::get_class() as *const _ as usize,
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
        object::object_has_own_property as usize,
        object::object_property_is_enumerable as usize,
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
        error::uri_error_constructor as usize,
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
        promise::promise_constructor as _,
        promise::promise_then as _,
        promise::promise_catch as _,
        promise::promise_finally as _,
        promise::promise_resolve as _,
        promise::promise_reject as _,
        promise::promise_static_resolve as _,
        promise::promise_static_reject as _,
        promise::promise_static_race as _,
        promise::promise_static_all as _,
        promise::promise_static_all_settled as _,
        promise::promise_static_any as _,
        generator::generator_next as _,
        generator::generator_iterator as _,
        generator::generator_return as _,
        generator::generator_throw as _,
        array_buffer::array_buffer_constructor as _,
        array_buffer::array_buffer_byte_length as _,
        array_buffer::array_buffer_slice as _,
        JsArrayBuffer::get_class() as *const _ as usize,
        JsDataView::get_class() as *const _ as usize,
        data_view::data_view_constructor as _,
        data_view::data_view_prototype_buffer as _,
        data_view::data_view_prototype_byte_length as _,
        data_view::data_view_prototype_byte_offset as _,
        data_view::data_view_prototype_get::<u8> as _,
        data_view::data_view_prototype_get::<u16> as _,
        data_view::data_view_prototype_get::<u32> as _,
        data_view::data_view_prototype_get::<i8> as _,
        data_view::data_view_prototype_get::<i16> as _,
        data_view::data_view_prototype_get::<i32> as _,
        data_view::data_view_prototype_get::<f32> as _,
        data_view::data_view_prototype_get::<f64> as _,
        data_view::data_view_prototype_set::<u8> as _,
        data_view::data_view_prototype_set::<u16> as _,
        data_view::data_view_prototype_set::<u32> as _,
        data_view::data_view_prototype_set::<i8> as _,
        data_view::data_view_prototype_set::<i16> as _,
        data_view::data_view_prototype_set::<i32> as _,
        data_view::data_view_prototype_set::<f32> as _,
        data_view::data_view_prototype_set::<f64> as _,
        weak_ref::JsWeakRef::get_class() as *const _ as _,
        weak_ref::weak_ref_constructor as _,
        weak_ref::weak_ref_prototype_deref as _,
        WeakSlot::deserialize as _,
        WeakSlot::allocate as _,
        boolean::boolean_constructor as _,
        boolean::boolean_to_string as _,
        boolean::boolean_value_of as _,
        boolean::BooleanObject::get_class() as *const _ as _,
        date::date_constructor as _,
        date::date_to_string as _,
        date::Date::get_class() as *const _ as _,
        date::date_now as _,
        date::date_set_date as _,
        date::date_set_full_year as _,
        date::date_set_hours as _,
        date::date_set_milliseconds as _,
        date::date_set_minutes as _,
        date::date_set_month as _,
        date::date_set_seconds as _,
        date::date_set_year as _,
        date::date_set_time as _,
        date::date_set_utc_date as _,
        date::date_set_utc_full_year as _,
        date::date_set_utc_hours as _,
        date::date_set_utc_minutes as _,
        date::date_set_utc_month as _,
        date::date_set_utc_seconds as _,
        date::date_get_date as _,
        date::date_get_day as _,
        date::date_get_full_year as _,
        date::date_get_hours as _,
        date::date_get_milliseconds as _,
        date::date_get_minutes as _,
        date::date_get_month as _,
        date::date_get_seconds as _,
        date::date_get_seconds as _,
        date::date_get_time as _,
        date::date_get_year as _,
        date::date_get_utc_date as _,
        date::date_get_utc_day as _,
        date::date_get_utc_full_year as _,
        date::date_get_utc_hours as _,
        date::date_get_utc_minutes as _,
        date::date_get_utc_milliseconds as _,
        date::date_get_utc_month as _,
        date::date_get_utc_seconds as _,
        date::date_to_json as _,
        date::date_to_time_string as _,
        date::date_value_of as _,
        date::date_to_gmt_string as _,
        date::date_to_iso_string as _,
        date::date_to_utc_string as _,
        date::date_to_date_string as _,
        date::date_parse as _,
        date::date_utc as _,
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

pub fn get_length(ctx: GcPointer<Context>, val: &mut GcPointer<JsObject>) -> Result<u32, JsValue> {
    if std::ptr::eq(val.class(), JsArray::get_class()) {
        return Ok(val.indexed.length());
    }
    let len = val.get(ctx, S_LENGTH.intern())?;
    len.to_uint32(ctx)
}

/// Convert JS object to JS property descriptor
pub fn to_property_descriptor(
    ctx: GcPointer<Context>,
    target: JsValue,
) -> Result<PropertyDescriptor, JsValue> {
    if !target.is_jsobject() {
        return Err(JsValue::new(
            ctx.new_type_error("ToPropertyDescriptor requires Object argument"),
        ));
    }

    let mut attr: u32 = DEFAULT;
    let stack = ctx.shadowstack();
    letroot!(obj = stack, target.get_jsobject());
    let mut value = JsValue::encode_undefined_value();
    let mut getter = JsValue::encode_undefined_value();
    let mut setter = JsValue::encode_undefined_value();

    {
        let sym = S_ENUMERABLE.intern();
        if obj.has_property(ctx, sym) {
            let enumerable = obj.get(ctx, sym)?.to_boolean();
            if enumerable {
                attr = (attr & !UNDEF_ENUMERABLE) | ENUMERABLE;
            } else {
                attr &= !UNDEF_ENUMERABLE;
            }
        }
    }
    {
        let sym = S_CONFIGURABLE.intern();
        if obj.has_property(ctx, sym) {
            let configurable = obj.get(ctx, sym)?.to_boolean();
            if configurable {
                attr = (attr & !UNDEF_CONFIGURABLE) | CONFIGURABLE;
            } else {
                attr &= !UNDEF_CONFIGURABLE;
            }
        }
    }

    {
        let sym = S_VALUE.intern();
        if obj.has_property(ctx, sym) {
            value = obj.get(ctx, sym)?;
            attr |= DATA;
            attr &= !UNDEF_VALUE;
        }
    }

    {
        let sym = S_WRITABLE.intern();
        if obj.has_property(ctx, sym) {
            let writable = obj.get(ctx, sym)?.to_boolean();
            attr |= DATA;
            attr &= !UNDEF_WRITABLE;
            if writable {
                attr |= WRITABLE;
            }
        }
    }
    {
        let sym = S_GET.intern();
        if obj.has_property(ctx, sym) {
            let r = obj.get(ctx, sym)?;
            if !r.is_callable() && !r.is_undefined() {
                return Err(JsValue::new(
                    ctx.new_type_error("property 'get' is not callable"),
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
        let sym = S_SET.intern();
        if obj.has_property(ctx, sym) {
            let r = obj.get(ctx, sym)?;
            if !r.is_callable() && !r.is_undefined() {
                return Err(JsValue::new(
                    ctx.new_type_error("property 'set' is not callable"),
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
            ctx.new_type_error("invalid property descriptor object"),
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

pub(crate) fn module_load(
    mut ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let name = args.at(0).to_string(ctx)?;
    let rel_path = unsafe { (*ctx.stack.current).code_block.unwrap().path.clone() };
    let _is_js_load = (name.starts_with("./")
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
            return Err(JsValue::new(ctx.new_reference_error(format!(
                "Module '{}' not found: '{}'",
                path.display(),
                e
            ))))
        }
        Ok(path) => path,
    };
    let stack = ctx.shadowstack();
    letroot!(module_object = stack, JsObject::new_empty(ctx));
    let mut exports = JsObject::new_empty(ctx);
    module_object.put(ctx, S_EXPORTS.intern(), JsValue::new(exports), false)?;
    let mut args = [JsValue::new(*module_object)];
    letroot!(
        args = stack,
        Arguments::new(JsValue::encode_undefined_value(), &mut args)
    );
    if let Some(module) = ctx.modules().get(&spath).copied() {
        match module {
            ModuleKind::Initialized(x) => {
                return Ok(JsValue::new(x));
            }
            ModuleKind::NativeUninit(init) => {
                let mut module = *module_object;
                init(ctx, module)?;
                ctx.modules()
                    .insert(spath.clone(), ModuleKind::Initialized(module));

                return Ok(JsValue::new(module));
            }
        }
    }
    if !path.exists() {
        return Err(JsValue::new(
            ctx.new_type_error(format!("Module '{}' not found", spath)),
        ));
    }
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(e) => {
            return Err(JsValue::new(ctx.new_type_error(format!(
                "Failed to read module '{}': {}",
                spath,
                e.to_string()
            ))));
        }
    };
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    let module_fun = ctx.compile_module(&spath, &name, &source)?;
    let mut module_fun = module_fun.get_jsobject();
    module_fun
        .as_function_mut()
        .call(ctx, &mut args, JsValue::encode_undefined_value())?;
    ctx.modules()
        .insert(spath.clone(), ModuleKind::Initialized(*module_object));
    Ok(JsValue::new(*module_object))
}

pub fn to_index(ctx: GcPointer<Context>, val: JsValue) -> Result<usize, JsValue> {
    let value = if val.is_undefined() {
        JsValue::new(0)
    } else {
        val
    };
    let res = value.to_number(ctx)?;
    if res < 0.0 {
        return Err(JsValue::new(ctx.new_range_error("Negative index")));
    }
    if res >= 9007199254740991.0 {
        return Err(JsValue::new(ctx.new_range_error(
            "The value given for the index must be between 0 and 2 ^ 53 - 1",
        )));
    }
    Ok(res as _)
}

pub fn define_lazy_property(
    ctx: GcPointer<Context>,
    mut object: GcPointer<JsObject>,
    name: Symbol,
    init: Rc<dyn Fn() -> PropertyDescriptor>,
    throwable: bool,
) -> Result<(), JsValue> {
    let c = init.clone();
    let getter = JsClosureFunction::new(
        ctx,
        S_INIT_PROPERTY.intern(),
        move |ctx, args| {
            let desc = init();
            let mut this = args.this.to_object(ctx)?;
            this.define_own_property(ctx, name, &desc, throwable)?;
            this.get(ctx, name)
        },
        0,
    );
    let setter = JsClosureFunction::new(
        ctx,
        S_INIT_PROPERTY.intern(),
        move |ctx, args| {
            let mut this = args.this.to_object(ctx)?;
            let desc = c();
            this.define_own_property(ctx, name, &desc, throwable)?;
            this.put(ctx, name, args.at(0), true)?;
            Ok(JsValue::encode_undefined_value())
        },
        0,
    );
    let desc = AccessorDescriptor::new(JsValue::new(getter), JsValue::new(setter), C | E);
    object.define_own_property(ctx, name, &*desc, throwable)?;
    Ok(())
}
