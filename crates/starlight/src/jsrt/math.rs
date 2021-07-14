use crate::{prelude::*, vm::context::Context};
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
        let y = args.at(1).to_number(ctx)?;
        Ok(JsValue::new(num.log(y)))
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
impl GcPointer<Context> {
    pub(crate) fn init_math_in_global_object(mut self) {
        let mut init = || -> Result<(), JsValue> {
            let mut math = JsObject::new_empty(self);
            /* let f = JsNativeFunction::new(self, "trunc".intern(), math_trunc, 1);
            math.put(self, "trunc".intern(), JsValue::new(f), false)?;
            let f = JsNativeFunction::new(self, "floor".intern(), math_floor, 1);
            math.put(self, "floor".intern(), JsValue::new(f), false)?;*/
            def_native_method!(self, math, trunc, math_trunc, 1)?;
            def_native_method!(self, math, floor, math_floor, 1)?;
            def_native_method!(self, math, log, math_log, 2)?;
            def_native_method!(self, math, sin, math_sin, 1)?;
            def_native_method!(self, math, cos, math_cos, 1)?;
            def_native_method!(self, math, ceil, math_ceil, 1)?;
            def_native_method!(self, math, exp, math_exp, 1)?;
            def_native_method!(self, math, abs, math_abs, 1)?;
            def_native_method!(self, math, random, math_random, 0)?;
            def_native_method!(self, math, sqrt, math_sqrt, 1)?;
            math.put(
                self,
                "PI".intern(),
                JsValue::new(std::f64::consts::PI),
                false,
            )?;
            self.global_object()
                .put(self, "Math".intern(), JsValue::new(math), false)?;
            let source = include_str!("../builtins/Math.js");

            self.eval_internal(Some("../builtins/Math.js"), false, source, true)?;

            Ok(())
        };

        match init() {
            Ok(_) => (),
            Err(e) => {
                eprintln!("{}", e.to_string(self).unwrap_or_else(|_| panic!()));
                unreachable!();
            }
        }
    }
}
