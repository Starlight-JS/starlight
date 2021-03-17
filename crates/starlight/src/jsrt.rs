use std::collections::HashMap;

use crate::{
    heap::cell::{GcPointer, WeakRef},
    vm::{
        arguments::Arguments, arguments::JsArguments, array::JsArray, array_storage::ArrayStorage,
        attributes::*, code_block::CodeBlock, error::*, function::*, global::JsGlobal,
        indexed_elements::IndexedElements, interpreter::SpreadValue, object::*,
        property_descriptor::*, string::*, structure::*, symbol_table::*, value::*, Runtime,
    },
};

pub mod array;
pub mod error;
pub mod function;
pub mod object;

use array::*;
use error::*;
use function::*;
use wtf_rs::keep_on_stack;
#[no_mangle]
pub fn print(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    for i in 0..args.size() {
        let value = args.at(i);
        let string = value.to_string(rt)?;
        print!("{}", string);
    }
    println!();
    Ok(JsValue::encode_f64_value(args.size() as _))
}

impl Runtime {
    pub(crate) fn init_builtin(&mut self) {
        let func = JsNativeFunction::new(self, "print".intern(), print, 0);
        self.global_object()
            .put(
                self,
                "print".intern(),
                JsValue::encode_object_value(func),
                false,
            )
            .unwrap_or_else(|_| unreachable!());
    }
    pub(crate) fn init_func(&mut self, obj_proto: GcPointer<JsObject>) {
        let _structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let name = "Function".intern();
        let mut func_proto = JsNativeFunction::new(self, name, function_prototype, 1);
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
        let f = JsNativeFunction::new(self, "toString".intern(), function_bind, 0);
        let name = "bind".intern();
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
        let mut proto = JsObject::new(self, structure, JsObject::get_class(), ObjectTag::Ordinary);
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
        let mut proto = JsObject::new(self, structure, JsError::get_class(), ObjectTag::Ordinary);
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
                structure,
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
                structure,
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
                structure,
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
                structure,
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
                structure,
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
use crate::heap::snapshot::deserializer::*;
use once_cell::sync::Lazy;

pub static VM_NATIVE_REFERENCES: Lazy<&'static [usize]> = Lazy::new(|| {
    Box::leak(Box::new([
        /* deserializer functions */
        // following GcPointer and WeakRef method references is obtained from `T = u8`
        // but they should be the same for all types that is allocated in GC heap.
        Vec::<crate::heap::cell::GcPointer<crate::vm::structure::Structure>>::deserialize as _,
        Vec::<crate::heap::cell::GcPointer<crate::vm::structure::Structure>>::allocate as _,
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
        Structure::deserialize as _,
        Structure::allocate as _,
        crate::vm::structure::Table::deserialize as _,
        crate::vm::structure::Table::allocate as _,
        SpreadValue::allocate as _,
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
        object::object_constructor as usize,
        object::object_create as usize,
        object::object_to_string as usize,
        array::array_ctor as usize,
        array::array_from as usize,
        array::array_is_array as usize,
        array::array_join as usize,
        array::array_of as usize,
        array::array_pop as usize,
        array::array_push as usize,
        array::array_to_string as usize,
        error::error_constructor as usize,
        error::error_to_string as usize,
        error::eval_error_constructor as usize,
        error::range_error_constructor as usize,
        error::reference_error_constructor as usize,
        error::syntax_error_constructor as usize,
        error::type_error_constructor as usize,
        print as usize,
    ]))
});
