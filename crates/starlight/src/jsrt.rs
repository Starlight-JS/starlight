use crate::{
    heap::cell::GcPointer,
    vm::{
        attributes::*, error::*, function::*, object::*, property_descriptor::*, string::*,
        structure::*, symbol_table::*, value::*, Runtime,
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

impl Runtime {
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
