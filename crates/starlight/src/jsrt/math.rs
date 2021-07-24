use core::f64;
use std::intrinsics::unlikely;

use crate::{
    prelude::*,
    vm::{builder::Builtin, context::Context},
};
pub fn math_abs(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        if args.at(0).is_int32() {
            return Ok(JsValue::new(args.at(0).get_int32().abs()));
        }
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.abs()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_acos(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.acos()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_asin(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.asin()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_atan(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.atan()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}
pub fn math_atan2(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() < 1 {
        let num = args.at(0).to_number(ctx)?;
        let x = args.at(1).to_number(ctx);
        Ok(JsValue::new(num.atan2(x?)))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_ceil(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.ceil()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_cos(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.cos()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_sin(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.sin()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_exp(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.exp()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_floor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.floor()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}
pub fn math_trunc(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.trunc()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_log(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 0 {
        let num = args.at(0).to_number(ctx)?;
        Ok(JsValue::new(num.ln()))
    } else {
        Ok(JsValue::encode_nan_value())
    }
}

pub fn math_random(_ctx: GcPointer<Context>, _args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(rand::random::<f64>()))
}
pub fn math_sqrt(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(args.at(0).to_number(ctx)?.sqrt()))
}

pub fn math_pow(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let left = args.at(0).to_number(ctx)?;
    let right = args.at(1).to_number(ctx)?;
    Ok(JsValue::new(left.powf(right)))
}

pub fn math_acosh(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue,JsValue>{
    let left = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(left.acosh()))
}

pub fn math_asinh(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue,JsValue>{
    let left = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(left.asinh()))
}

pub fn math_atanh(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue>{
    let left = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(left.atanh()))
}

pub fn math_cbrt(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue>{
    let left = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(left.cbrt()))
}

pub fn math_clz32(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue>{
    let left = args.at(0).to_uint32(ctx)?;
    Ok(JsValue::new(left.leading_zeros()))
}

pub fn math_expm1(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue,JsValue> {
    let left = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(left.exp()-1.0))
}

pub fn math_fround(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue>{
    let left = args.at(0).to_f32(ctx)?;
    Ok(JsValue::new(left))
}

pub fn math_hypot(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue,JsValue> {
    let mut sum = 0f64;
    for index in 0..args.size() {
        let number = args.at(index).to_number(ctx)?;
        sum += number * number;
    }
    Ok(JsValue::new(sum.sqrt()))
}
pub fn math_imul(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue,JsValue> {
    let left = args.at(0).to_uint32(ctx)?;
    let right = args.at(1).to_uint32(ctx)?;
    Ok(JsValue::new(left.wrapping_mul(right)))
}

pub fn math_log10(ctx: GcPointer<Context>, args:&Arguments) -> Result<JsValue,JsValue> {
    let number = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(number.log10()))
}

pub fn math_log1p(ctx: GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue> {
    let number = args.at(0).to_number(ctx)?;
    Ok(JsValue::new((number+1.0).ln()))
}

pub fn math_log2(ctx:GcPointer<Context>, args:&Arguments) -> Result<JsValue, JsValue> {
    let number = args.at(0).to_number(ctx)?;
    Ok(JsValue::new(number.log2()))
}

pub fn math_sign(ctx:GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let number = args.at(0).to_number(ctx)?;
    if unlikely(number ==0.0 || number==-0.0){
        return Ok(JsValue::new(0));
    }
    Ok(JsValue::new(number.signum()))
}

pub struct Math;

impl Builtin for Math {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let mut math = JsObject::new_empty(ctx);

        def_native_method!(ctx, math, abs, math_abs, 1)?;
        def_native_method!(ctx, math, acos, math_acos, 1)?;
        def_native_method!(ctx, math, trunc, math_trunc, 1)?;
        def_native_method!(ctx, math, floor, math_floor, 1)?;
        def_native_method!(ctx, math, log, math_log, 2)?;
        def_native_method!(ctx, math, sin, math_sin, 1)?;
        def_native_method!(ctx, math, cos, math_cos, 1)?;
        def_native_method!(ctx, math, ceil, math_ceil, 1)?;
        def_native_method!(ctx, math, exp, math_exp, 1)?;
        def_native_method!(ctx, math, random, math_random, 0)?;
        def_native_method!(ctx, math, sqrt, math_sqrt, 1)?;
        def_native_method!(ctx, math, pow, math_pow, 2)?;
        def_native_method!(ctx, math, asin, math_asin, 1)?;
        def_native_method!(ctx, math, atan, math_atan, 1)?;
        def_native_method!(ctx, math, atan2, math_atan2, 1)?;
        def_native_method!(ctx, math, ceil, math_ceil,1)?;
        def_native_method!(ctx, math, acosh, math_acosh, 1)?;
        def_native_method!(ctx, math, asinh, math_asinh, 1)?;


        def_native_property!(ctx, math, E, f64::consts::E)?;
        def_native_property!(ctx, math, LN10, f64::consts::LN_10)?;
        def_native_property!(ctx, math, LN2, f64::consts::LN_2)?;
        def_native_property!(ctx, math, LOG10E, f64::consts::LOG10_E)?;
        def_native_property!(ctx, math, LOG2E, f64::consts::LOG2_E)?;
        def_native_property!(ctx, math, PI, f64::consts::PI)?;
        def_native_property!(ctx, math, SQRT1_2, f64::consts::FRAC_1_SQRT_2)?;
        def_native_property!(ctx, math, SQRT2, f64::consts::SQRT_2)?;




        def_native_property!(ctx, math, PI, std::f64::consts::PI)?;

        let mut global_object = ctx.global_object();

        def_native_property!(ctx, global_object, Math, math)?;

        let source = include_str!("../builtins/Math.js");
        ctx.eval_internal(Some("../builtins/Math.js"), false, source, true)?;

        Ok(())
    }
}
