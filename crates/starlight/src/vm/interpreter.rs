use std::hint::unreachable_unchecked;

use super::{
    arguments::*, array::*, attributes::*, function::JsVMFunction, object::*,
    property_descriptor::*, slot::*, structure::*, symbol_table::*, value::*, Runtime,
};

impl Runtime {
    pub(crate) fn perform_vm_call(
        &mut self,
        func: &JsVMFunction,
        env: JsValue,
        args_: &Arguments,
    ) -> Result<JsValue, JsValue> {
        let scope = unsafe { env.get_object().downcast_unchecked::<JsObject>() };
        let structure = Structure::new_indexed(self, Some(scope), false);

        let mut nscope = JsObject::new(self, structure, JsObject::get_class(), ObjectTag::Ordinary);

        let mut i = 0;
        for p in func.code.params.iter() {
            let _ = nscope
                .put(self, *p, args_.at(i), false)
                .unwrap_or_else(|_| unsafe { unreachable_unchecked() });
            i += 1;
        }

        if let Some(rest) = func.code.rest_param {
            let mut args_arr = JsArray::new(self, args_.size() as u32 - i as u32);
            let mut ix = 0;
            for _ in i..args_.size() {
                args_arr.put_indexed_slot(self, ix, args_.at(ix as _), &mut Slot::new(), false)?;
                ix += 1;
            }
            nscope.put(self, rest, JsValue::encode_object_value(args_arr), false)?;
        }
        for val in func.code.variables.iter() {
            nscope.define_own_property(
                self,
                *val,
                &*DataDescriptor::new(JsValue::encode_undefined_value(), W | C | E),
                false,
            )?;
        }

        let mut args = JsArguments::new(self, nscope, &func.code.params, args_.size() as _);

        for k in i..args_.size() {
            args.put(self, Symbol::Index(k as _), args_.at(k), false)?;
        }

        let _ = nscope.put(
            self,
            "arguments".intern(),
            JsValue::encode_object_value(args),
            false,
        )?;

        let this = if func.code.strict && !args_.this.is_object() {
            JsValue::encode_undefined_value()
        } else {
            if args_.this.is_undefined() {
                JsValue::encode_object_value(self.global_object())
            } else {
                args_.this
            }
        };

        todo!()
    }
}
