use std::intrinsics::unlikely;

use crate::prelude::*;
use crate::vm::function::*;
pub(crate) fn init_generator(rt: &mut Runtime, _obj_proto: GcPointer<JsObject>) {
    let mut init = || -> Result<(), JsValue> {
        let f = Some(rt.global_data().func_prototype.unwrap());
        let generator_structure = Structure::new_indexed(rt, f, false);

        let mut generator = JsObject::new(
            rt,
            &generator_structure,
            JsObject::get_class(),
            ObjectTag::Ordinary,
        );

        def_native_method!(rt, generator, next, generator_next, 0)?;
        let iter = JsNativeFunction::new(
            rt,
            "Symbol.iterator".intern().private(),
            generator_iterator,
            0,
        );
        generator.put(
            rt,
            "Symbol.iterator".intern().private(),
            JsValue::new(iter),
            false,
        )?;
        rt.global_data.generator_prototype = Some(generator);
        rt.global_data.generator_structure =
            Some(Structure::new_indexed(rt, Some(generator), false));
        Ok(())
    };

    match init() {
        Ok(_) => (),
        Err(_) => panic!("Failed to initialize generator object"),
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
