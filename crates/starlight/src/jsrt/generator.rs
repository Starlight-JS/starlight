use std::intrinsics::unlikely;

use crate::prelude::*;
use crate::vm::builder::Builtin;
use crate::vm::{context::Context, function::*};

impl Builtin for JsGeneratorFunction {
    fn native_references() -> Vec<usize> {
        vec![
            generator_next as _,
            generator_iterator as _,
            generator_return as _,
            generator_throw as _,
        ]
    }

    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let f = Some(ctx.global_data.func_prototype.unwrap());
        let generator_structure = Structure::new_indexed(ctx, f, false);

        let mut generator = JsObject::new(
            ctx,
            &generator_structure,
            JsObject::class(),
            ObjectTag::Ordinary,
        );

        def_native_method!(ctx, generator, next, generator_next, 0)?;
        def_native_method!(ctx, generator, throw, generator_throw, 0)?;
        def_native_method!(ctx, generator, r#return, generator_return, 0)?;

        let iter = JsNativeFunction::new(
            ctx,
            "Symbol.iterator".intern().private(),
            generator_iterator,
            0,
        );
        generator.put(
            ctx,
            "Symbol.iterator".intern().private(),
            JsValue::new(iter),
            false,
        )?;
        ctx.global_data.generator_prototype = Some(generator);
        ctx.global_data.generator_structure =
            Some(Structure::new_indexed(ctx, Some(generator), false));
        Ok(())
    }
}

pub fn generator_iterator(_: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(args.this)
}

pub fn generator_next(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if unlikely(!this.is_class(JsGeneratorFunction::class())) {
        return Err(JsValue::new(ctx.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        ctx,
        JsValue::new(this),
        args,
        GeneratorMagic::Next,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(ctx);
        ret_obj.put(ctx, "value".intern(), ret, false)?;
        ret_obj.put(ctx, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}

pub fn generator_return(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if unlikely(!this.is_class(JsGeneratorFunction::class())) {
        return Err(JsValue::new(ctx.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        ctx,
        JsValue::new(this),
        args,
        GeneratorMagic::Return,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(ctx);
        ret_obj.put(ctx, "value".intern(), ret, false)?;
        ret_obj.put(ctx, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}

pub fn generator_throw(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if unlikely(!this.is_class(JsGeneratorFunction::class())) {
        return Err(JsValue::new(ctx.new_type_error("not generator function")));
    }
    let mut done = 0;
    let mut ret = js_generator_next(
        ctx,
        JsValue::new(this),
        args,
        GeneratorMagic::Throw,
        &mut done,
    )?;
    if done != 2 {
        let mut ret_obj = JsObject::new_empty(ctx);
        ret_obj.put(ctx, "value".intern(), ret, false)?;
        ret_obj.put(ctx, "done".intern(), JsValue::new(done != 0), false)?;
        ret = JsValue::new(ret_obj);
    }
    Ok(ret)
}
