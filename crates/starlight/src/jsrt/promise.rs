use crate::vm::arguments::Arguments;
use crate::vm::promise::JsPromise;
use crate::vm::string::JsString;
use crate::vm::value::JsValue;
use crate::vm::Runtime;

pub fn promise_constructor(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let func = args.at(0);
    if !func.is_callable() {
        Err(JsValue::encode_object_value(JsString::new(
            vm,
            "arg 1 should be a function",
        )))
    } else {
        JsPromise::new(vm, func)
    }
}

fn with_prom<C: FnOnce(&mut Runtime, &Arguments, &mut JsPromise) -> Result<JsValue, JsValue>>(
    vm: &mut Runtime,
    args: &Arguments,
    consumer: C,
) -> Result<JsValue, JsValue> {
    let this = args.this;
    if !this.is_object() {
        Err(JsValue::encode_object_value(JsString::new(
            vm,
            "method not called on a Promise",
        )))
    } else {
        let mut this_obj = this.get_jsobject();

        if !this_obj.is_class(JsPromise::get_class()) {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "method not called on a Promise",
            )))
        } else {
            consumer(vm, args, this_obj.as_promise_mut())
        }
    }
}

pub fn promise_then(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    // onResolved and onRejected arg, both optional but callable if has val
    with_prom(vm, args, |vm, args, prom| {
        let mut on_resolved_opt = None;
        let mut on_rejected_opt = None;

        if args.size() >= 1 {
            let resolved = args.at(0);
            if !resolved.is_callable() {
                return Err(JsValue::encode_object_value(JsString::new(
                    vm,
                    "resolved argument is not a Function",
                )));
            }
            on_resolved_opt = Some(resolved);
        }
        if args.size() >= 2 {
            let rejected = args.at(1);
            if !rejected.is_callable() {
                return Err(JsValue::encode_object_value(JsString::new(
                    vm,
                    "rejected argument is not a Function",
                )));
            }
            on_rejected_opt = Some(rejected);
        }

        prom.then(vm, on_resolved_opt, on_rejected_opt, None)
    })
}

pub fn promise_catch(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(vm, args, |vm, args, prom| {
        if args.size() == 1 {
            let rejected = args.at(0);
            if !rejected.is_callable() {
                Err(JsValue::encode_object_value(JsString::new(
                    vm,
                    "rejected argument is not a Function",
                )))
            } else {
                prom.then(vm, None, Some(rejected), None)
            }
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "resolved argument is not a Function",
            )))
        }
    })
}

pub fn promise_finally(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(vm, args, |vm, args, prom| {
        if args.size() == 1 {
            let finally = args.at(0);
            if !finally.is_callable() {
                Err(JsValue::encode_object_value(JsString::new(
                    vm,
                    "finally argument is not a Function",
                )))
            } else {
                prom.then(vm, None, None, Some(finally))
            }
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "resolved argument is not a Function",
            )))
        }
    })
}

pub fn promise_resolve(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(vm, args, |vm, args, prom| {
        if args.size() == 1 {
            prom.resolve(vm, args.this, args.at(0))?;
            Ok(JsValue::encode_undefined_value())
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "resolve should be called with a single argument",
            )))
        }
    })
}

pub fn promise_reject(vm: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    with_prom(vm, args, |vm, args, prom| {
        if args.size() == 1 {
            prom.reject(vm, args.this, args.at(0))?;
            Ok(JsValue::encode_undefined_value())
        } else {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "reject should be called with a single argument",
            )))
        }
    })
}

pub fn promise_static_resolve(_vm: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!();
}

pub fn promise_static_reject(_vm: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!();
}

pub fn promise_static_race(_vm: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!();
}

pub fn promise_static_all(_vm: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!();
}

pub fn promise_static_all_settled(
    _vm: &mut Runtime,
    _args: &Arguments,
) -> Result<JsValue, JsValue> {
    todo!();
}

pub fn promise_static_any(_vm: &mut Runtime, _args: &Arguments) -> Result<JsValue, JsValue> {
    todo!();
}
