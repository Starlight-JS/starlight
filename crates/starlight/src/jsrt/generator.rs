use std::intrinsics::unlikely;

use crate::prelude::*;
use crate::vm::function::*;

impl Runtime {
    pub(crate) fn init_generator_in_global_data(&mut self, _obj_proto: GcPointer<JsObject>) {
        let mut init = || -> Result<(), JsValue> {
            let f = Some(self.global_data().func_prototype.unwrap());
            let generator_structure = Structure::new_indexed(self, f, false);

            let mut generator = JsObject::new(
                self,
                &generator_structure,
                JsObject::get_class(),
                ObjectTag::Ordinary,
            );

            def_native_method!(self, generator, next, generator_next, 0)?;
            def_native_method!(self, generator, throw, generator_throw, 0)?;
            def_native_method!(self, generator, r#return, generator_return, 0)?;
            let iter = JsNativeFunction::new(
                self,
                "Symbol.iterator".intern().private(),
                generator_iterator,
                0,
            );
            generator.put(
                self,
                "Symbol.iterator".intern().private(),
                JsValue::new(iter),
                false,
            )?;
            self.global_data.generator_prototype = Some(generator);
            self.global_data.generator_structure =
                Some(Structure::new_indexed(self, Some(generator), false));
            Ok(())
        };

        match init() {
            Ok(_) => (),
            Err(_) => panic!("Failed to initialize generator object"),
        }
    }
}

pub fn generator_iterator(_: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn generator_next(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if unlikely(!this.is_class(JsGeneratorFunction::get_class())) {
        return Err(JsValue::new(rt.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        rt,
        JsValue::new(this),
        args,
        GeneratorMagic::Next,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(rt);
        ret_obj.put(rt, "value".intern(), ret, false)?;
        ret_obj.put(rt, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}

pub fn generator_return(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if unlikely(!this.is_class(JsGeneratorFunction::get_class())) {
        return Err(JsValue::new(rt.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        rt,
        JsValue::new(this),
        args,
        GeneratorMagic::Return,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(rt);
        ret_obj.put(rt, "value".intern(), ret, false)?;
        ret_obj.put(rt, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}

pub fn generator_throw(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if unlikely(!this.is_class(JsGeneratorFunction::get_class())) {
        return Err(JsValue::new(rt.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        rt,
        JsValue::new(this),
        args,
        GeneratorMagic::Throw,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(rt);
        ret_obj.put(rt, "value".intern(), ret, false)?;
        ret_obj.put(rt, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}
