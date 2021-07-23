use crate::constant::S_PROMISE;
use crate::gc::cell::GcPointer;
use crate::prelude::JsArray;
use crate::vm::arguments::Arguments;
use crate::vm::builder::Builtin;
use crate::vm::class::JsClass;
use crate::vm::context::Context;
use crate::vm::function::JsNativeFunction;
use crate::vm::object::JsObject;
use crate::vm::promise::{JsPromise, TrackingMode};
use crate::vm::string::JsString;
use crate::vm::symbol_table::Internable;
use crate::vm::value::JsValue;

pub fn promise_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let func = args.at(0);
    if !func.is_callable() {
        Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "arg 1 should be a function",
        )))
    } else {
        JsPromise::new(ctx, func)
    }
}

fn with_prom<
    C: FnOnce(GcPointer<Context>, &Arguments, &mut JsPromise) -> Result<JsValue, JsValue>,
>(
    ctx: GcPointer<Context>,
    args: &Arguments,
    consumer: C,
) -> Result<JsValue, JsValue> {
    let this = args.this;
    if !this.is_object() {
        Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "method not called on a Promise",
        )))
    } else {
        let mut this_obj = this.get_jsobject();

        if !this_obj.is_class(JsPromise::class()) {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "method not called on a Promise",
            )))
        } else {
            consumer(ctx, args, this_obj.as_promise_mut())
        }
    }
}

pub fn promise_then(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    // onResolved and onRejected arg, both optional but callable if has val
    with_prom(ctx, args, |ctx, args, prom| {
        let mut on_resolved_opt = None;
        let mut on_rejected_opt = None;

        if args.size() >= 1 {
            let resolved = args.at(0);
            if !resolved.is_callable() {
                return Err(JsValue::encode_object_value(JsString::new(
                    ctx,
                    "resolved argument is not a Function",
                )));
            }
            on_resolved_opt = Some(resolved);
        }
        if args.size() >= 2 {
            let rejected = args.at(1);
            if !rejected.is_callable() {
                return Err(JsValue::encode_object_value(JsString::new(
                    ctx,
                    "rejected argument is not a Function",
                )));
            }
            on_rejected_opt = Some(rejected);
        }

        prom.then(ctx, on_resolved_opt, on_rejected_opt, None)
    })
}

pub fn promise_catch(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(ctx, args, |ctx, args, prom| {
        if args.size() == 1 {
            let rejected = args.at(0);
            if !rejected.is_callable() {
                Err(JsValue::encode_object_value(JsString::new(
                    ctx,
                    "rejected argument is not a Function",
                )))
            } else {
                prom.then(ctx, None, Some(rejected), None)
            }
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "resolved argument is not a Function",
            )))
        }
    })
}

pub fn promise_finally(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(ctx, args, |ctx, args, prom| {
        if args.size() == 1 {
            let finally = args.at(0);
            if !finally.is_callable() {
                Err(JsValue::encode_object_value(JsString::new(
                    ctx,
                    "finally argument is not a Function",
                )))
            } else {
                prom.then(ctx, None, None, Some(finally))
            }
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "resolved argument is not a Function",
            )))
        }
    })
}

pub fn promise_resolve(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(ctx, args, |ctx, args, prom| {
        if args.size() == 1 {
            prom.resolve(ctx, args.this, args.at(0))?;
            Ok(JsValue::encode_undefined_value())
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "resolve should be called with a single argument",
            )))
        }
    })
}

pub fn promise_reject(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(ctx, args, |ctx, args, prom| {
        if args.size() == 1 {
            prom.reject(ctx, args.this, args.at(0))?;
            Ok(JsValue::encode_undefined_value())
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "reject should be called with a single argument",
            )))
        }
    })
}

pub fn promise_static_resolve(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() != 1 {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "resolve needs exactly one argument",
        )));
    }

    let res = JsPromise::new_unresolving(ctx);
    let value = args.at(0);
    if let Ok(prom_val) = res {
        let mut prom_js_obj = prom_val.get_jsobject();
        let prom: &mut JsPromise = prom_js_obj.as_promise_mut();
        prom.resolve(ctx, prom_val, value)?;
    }
    res
}

pub fn promise_static_reject(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() != 1 {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "reject needs exactly one argument",
        )));
    }

    let res = JsPromise::new_unresolving(ctx);
    let value = args.at(0);
    if let Ok(prom_val) = res {
        let mut prom_js_obj = prom_val.get_jsobject();
        let prom: &mut JsPromise = prom_js_obj.as_promise_mut();
        prom.reject(ctx, prom_val, value)?;
    }
    res
}

pub fn promise_static_race(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 1
        || !args.at(0).is_jsobject()
        || !args.at(0).get_jsobject().is_class(JsArray::class())
    {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "race needs exactly one Array argument",
        )));
    }

    JsPromise::new_tracking(ctx, TrackingMode::Race, args.at(0))
}

pub fn promise_static_all(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 1
        || !args.at(0).is_jsobject()
        || !args.at(0).get_jsobject().is_class(JsArray::class())
    {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "all needs exactly one Array argument",
        )));
    }

    JsPromise::new_tracking(ctx, TrackingMode::All, args.at(0))
}

pub fn promise_static_all_settled(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if args.size() != 1
        || !args.at(0).is_jsobject()
        || !args.at(0).get_jsobject().is_class(JsArray::class())
    {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "allSettled needs exactly one Array argument",
        )));
    }

    JsPromise::new_tracking(ctx, TrackingMode::AllSettled, args.at(0))
}

pub fn promise_static_any(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() != 1
        || !args.at(0).is_jsobject()
        || !args.at(0).get_jsobject().is_class(JsArray::class())
    {
        return Err(JsValue::encode_object_value(JsString::new(
            ctx,
            "any needs exactly one Array argument",
        )));
    }

    JsPromise::new_tracking(ctx, TrackingMode::Any, args.at(0))
}

impl Builtin for JsPromise {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        // copied from file
        let mut constructor =
            JsNativeFunction::new(ctx, S_PROMISE.intern(), promise_constructor, 1);
        let mut global_object = ctx.global_object();
        let mut prototype = JsObject::new_empty(ctx);

        // members / proto
        def_native_method!(ctx, prototype, then, promise_then, 2)?;
        def_native_method!(ctx, prototype, catch, promise_catch, 1)?;
        def_native_method!(ctx, prototype, finally, promise_finally, 1)?;
        def_native_method!(ctx, prototype, resolve, promise_resolve, 1)?;
        def_native_method!(ctx, prototype, reject, promise_reject, 1)?;
        // statics
        def_native_method!(ctx, constructor, all, promise_static_all, 1)?;
        def_native_method!(ctx, constructor, allSettled, promise_static_all_settled, 1)?;
        def_native_method!(ctx, constructor, any, promise_static_any, 1)?;
        def_native_method!(ctx, constructor, race, promise_static_race, 1)?;
        def_native_method!(ctx, constructor, reject, promise_static_reject, 1)?;
        def_native_method!(ctx, constructor, resolve, promise_static_resolve, 1)?;

        def_native_property!(ctx, constructor, prototype, prototype)?;
        def_native_property!(ctx, prototype, constructor, constructor)?;
        def_native_property!(ctx, global_object, Promise, constructor)?;

        Ok(())
    }
}
