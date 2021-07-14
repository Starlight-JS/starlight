use super::arguments::*;
use super::context::Context;
use super::method_table::*;
use super::object::*;
use super::string::*;
use super::symbol_table::Internable;
use super::value::*;
use crate::gc::cell::GcPointer;
use crate::gc::cell::{Trace, Tracer};
use crate::gc::snapshot::deserializer::Deserializer;
use crate::gc::snapshot::serializer::SnapshotSerializer;
use crate::jsrt::get_length;
use crate::prelude::Symbol;
use crate::vm::array::JsArray;
use crate::vm::function::JsClosureFunction;
use crate::vm::structure::Structure;
use std::mem::ManuallyDrop;

pub enum TrackingMode {
    All,
    Race,
    AllSettled,
    Any,
}

pub struct JsPromise {
    // sub promises, added by calling then/catch/finally
    subs: Vec<(Option<JsValue>, Option<JsValue>, Option<JsValue>, JsValue)>, // then_func / catch_func / finally_func / sub_promise
    // when tracking we generate a Vec with results which we'll map to a result array later based on the TrackingMode
    tracking_mode: Option<TrackingMode>,
    tracking_results: Option<Vec<Option<Result<JsValue, JsValue>>>>,
    // resolution for this Promise
    resolution: Option<Result<JsValue, JsValue>>,
}

#[allow(non_snake_case)]
impl JsPromise {
    pub fn new(ctx: GcPointer<Context>, function_value: JsValue) -> Result<JsValue, JsValue> {
        let promise = Self::new_unresolving(ctx)?;

        // call the function passed to the promise constructor with a resolve and a reject arg

        let resolve_func = JsValue::encode_object_value(JsClosureFunction::new(
            ctx,
            "resolve".intern(),
            move |ctx, arguments| {
                // todo check one arg
                let resolution = arguments.at(0);
                let mut promise = promise;

                promise
                    .get_jsobject()
                    .as_promise_mut()
                    .resolve(ctx, promise, resolution)?;
                Ok(JsValue::encode_undefined_value())
            },
            1,
        ));
        let reject_func = JsValue::encode_object_value(JsClosureFunction::new(
            ctx,
            "reject".intern(),
            move |ctx, arguments| {
                let mut promise = promise;
                // reject promise here

                // todo check one arg
                let rejection = arguments.at(0);
                promise
                    .get_jsobject()
                    .as_promise_mut()
                    .reject(ctx, promise, rejection)?;
                Ok(JsValue::encode_undefined_value())
            },
            1,
        ));

        let mut args_vec = vec![resolve_func, reject_func];
        let mut arguments =
            Arguments::new(JsValue::encode_undefined_value(), args_vec.as_mut_slice());

        let res = function_value.get_jsobject().as_function_mut().call(
            ctx,
            &mut arguments,
            JsValue::encode_undefined_value(),
        );

        res.map(|_| promise)
    }
    pub fn new_unresolving(mut ctx: GcPointer<Context>) -> Result<JsValue, JsValue> {
        let proto = ctx
            .global_object()
            .get(ctx, "Promise".intern())?
            .to_object(ctx)?
            .get(ctx, "prototype".intern())?
            .to_object(ctx)?;

        let structure = Structure::new_indexed(ctx, Some(proto), false);
        let mut obj = JsObject::new(ctx, &structure, JsPromise::get_class(), ObjectTag::Ordinary);

        *obj.data::<JsPromise>() = ManuallyDrop::new(JsPromise {
            subs: vec![],
            tracking_mode: None,
            tracking_results: None,
            resolution: None,
        });
        Ok(JsValue::new(obj))
    }
    pub fn new_tracking(
       mut ctx: GcPointer<Context>,
        mode: TrackingMode,
        promises_array: JsValue,
    ) -> Result<JsValue, JsValue> {
        let proto = ctx
            .global_object()
            .get(ctx, "Promise".intern())?
            .to_object(ctx)?
            .get(ctx, "prototype".intern())?
            .to_object(ctx)?;

        if !promises_array.is_jsobject() {
            return Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "argument was not an Array",
            )));
        }
        let mut promises_array_object = promises_array.get_jsobject();
        if promises_array_object.tag() != ObjectTag::Array {
            return Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "argument was not an Array",
            )));
        }

        //promises_array_object.

        let length = array_util_get_length(ctx, &mut promises_array_object)?;

        let mut results = vec![None; length as usize];
        // let prom_array: JsArray = promises_array.get_jsobject().as_array();
        // todo for array.length add None to results vec
        // todo add handler to every promise with index, resolve that index in vec, check followup action based on mode

        let structure = Structure::new_indexed(ctx, Some(proto), false);
        let mut obj = JsObject::new(ctx, &structure, JsPromise::get_class(), ObjectTag::Ordinary);

        *obj.data::<JsPromise>() = ManuallyDrop::new(JsPromise {
            subs: vec![],
            tracking_mode: Some(mode),
            tracking_results: Some(results),
            resolution: None,
        });
        let promise_value = JsValue::new(obj);

        // for every prom add finally to set value in promise_value
        // todo do we have something like a for_each util somewhere?
        for x in 0..length {
            let sub_prom = array_util_get_value_at(ctx, &mut promises_array_object, x)?;
            let mut sub_prom_obj = sub_prom.get_jsobject();
            let sub_prom_jsprom: &mut JsPromise = sub_prom_obj.as_promise_mut();

            let outer_root = ctx.vm.add_persistent_root(promise_value);
            let sub_prom_root = ctx.vm.add_persistent_root(sub_prom);

            let sub_finally = JsValue::encode_object_value(JsClosureFunction::new(
                ctx,
                "finally_track".intern(),
                move |ctx, _args| {
                    // todo add single val to outer promise_value
                    let outer_prom = outer_root.get_value();
                    let sub_prom = sub_prom_root.get_value();
                    let sub_resolution = sub_prom.get_jsobject().as_promise().resolution;
                    outer_prom
                        .get_jsobject()
                        .as_promise_mut()
                        .resolve_single_tracked_resolution(
                            ctx,
                            outer_prom,
                            x,
                            sub_resolution.unwrap(),
                        )?;
                    Ok(JsValue::encode_null_value())
                },
                1,
            ));

            sub_prom_jsprom.then(ctx, None, None, Some(sub_finally))?;
        }

        Ok(promise_value)
    }
    fn resolve_single_tracked_resolution(
        &mut self,
        ctx: GcPointer<Context>,
        prom_this: JsValue,
        index: u32,
        resolution: Result<JsValue, JsValue>,
    ) -> Result<JsValue, JsValue> {
        // only do things if not yet settled
        if self.resolution.is_none() {
            // update single result
            let tracking_results = self.tracking_results.as_mut().unwrap();

            // replace val in vec
            let _ = std::mem::replace(&mut tracking_results[index as usize], Some(resolution));
            match self.tracking_mode.as_ref().unwrap() {
                TrackingMode::All => {
                    match resolution {
                        Ok(_) => {
                            // see if all proms have settled
                            if !tracking_results.contains(&None) {
                                // all have settled
                                // create array of results
                                let arr_value = JsValue::encode_object_value(JsArray::new(
                                    ctx,
                                    tracking_results.len() as u32,
                                ));
                                let mut arr_obj = arr_value.get_jsobject();

                                for x in 0..tracking_results.len() {
                                    // results should all be Some and Ok
                                    let res =
                                        tracking_results.get(x).unwrap().unwrap().ok().unwrap();
                                    array_util_set_value_at(ctx, &mut arr_obj, x as u32, res)?;
                                }
                                self.resolve(ctx, prom_this, arr_value)?;
                            }
                            Ok(JsValue::encode_null_value())
                        }
                        Err(err_res) => {
                            self.reject(ctx, prom_this, err_res)?;
                            Ok(JsValue::encode_null_value())
                        }
                    }
                }
                TrackingMode::Race => {
                    match resolution {
                        Ok(ok_res) => self.resolve(ctx, prom_this, ok_res)?,
                        Err(err_res) => self.reject(ctx, prom_this, err_res)?,
                    }
                    Ok(JsValue::encode_null_value())
                }
                TrackingMode::AllSettled => Ok(JsValue::encode_null_value()),
                TrackingMode::Any => Ok(JsValue::encode_null_value()),
            }
        } else {
            Ok(JsValue::encode_null_value())
        }
    }
    pub fn resolve(
        &mut self,
        ctx: GcPointer<Context>,
        prom_this: JsValue,
        resolution: JsValue,
    ) -> Result<(), JsValue> {
        self.do_resolve(ctx, prom_this, Ok(resolution))
    }
    pub fn reject(
        &mut self,
        ctx: GcPointer<Context>,
        prom_this: JsValue,
        rejection: JsValue,
    ) -> Result<(), JsValue> {
        self.do_resolve(ctx, prom_this, Err(rejection))
    }

    fn do_resolve(
        &mut self,
        mut ctx: GcPointer<Context>,
        prom_this: JsValue,
        resolution: Result<JsValue, JsValue>,
    ) -> Result<(), JsValue> {
        //println!("do_resolve, subs={}", self.subs.len());

        if self.resolution.is_some() {
            Err(JsValue::encode_object_value(JsString::new(
                ctx,
                "Promise was already resolved",
            )))
        } else {
            if resolution.is_ok() {
                // if promise is resolved with a promise we let that promise resolve this promise
                // as per spec this is not done for reject operations
                let resolution_value = resolution.ok().unwrap();
                if resolution_value.is_jsobject()
                    && resolution_value
                        .get_jsobject()
                        .is_class(JsPromise::get_class())
                {
                    // resolved with a promise
                    let mut resolution_object = resolution_value.get_jsobject();
                    let resolution_prom: &mut JsPromise = resolution_object.as_promise_mut();
                    // add self as sub to resolution prom
                    let pass_val_func = JsValue::encode_object_value(JsClosureFunction::new(
                        ctx,
                        "pass_val".intern(),
                        |_ctx, args| Ok(args.at(0)),
                        1,
                    ));
                    resolution_prom.subs.push((
                        Some(pass_val_func),
                        Some(pass_val_func),
                        None,
                        prom_this,
                    ));
                    // exit this do_resolve()
                    return Ok(());
                }
            }

            self.resolution = Some(resolution);

            // todo everything below needs to be in async job.. need to root persistent again... later
            // root prom_this
            let prom_root = ctx.vm.add_persistent_root(prom_this);

            ctx.schedule_async(move |ctx| {
                let prom_val = prom_root.get_value();
                let mut prom_js_object = prom_val.get_jsobject();
                let prom_self: &mut JsPromise = prom_js_object.as_promise_mut();

                if let Ok(ok_resolution) = prom_self.resolution.unwrap() {
                    for sub in &prom_self.subs {
                        // invoke 0, resolve 3
                        if let Some(jsFunc) = sub.0 {
                            let this = JsValue::encode_undefined_value();
                            let mut args_vec = vec![ok_resolution];
                            let mut args = Arguments::new(this, args_vec.as_mut_slice());
                            let sub_res = jsFunc
                                .get_jsobject()
                                .as_function_mut()
                                .call(ctx, &mut args, this);
                            let sub_res = sub
                                .3
                                .get_jsobject()
                                .as_promise_mut()
                                .do_resolve(ctx, sub.3, sub_res);
                            if sub_res.is_err() {
                                println!("could not resolve sub");
                            }
                        }
                    }
                } else {
                    let err_resolution = prom_self.resolution.unwrap().err().unwrap();
                    for sub in &prom_self.subs {
                        // invoke 1, resolve 3
                        if let Some(jsFunc) = sub.1 {
                            let this = JsValue::encode_undefined_value();
                            let mut args_vec = vec![err_resolution];
                            let mut args = Arguments::new(this, args_vec.as_mut_slice());
                            let sub_res = jsFunc
                                .get_jsobject()
                                .as_function_mut()
                                .call(ctx, &mut args, this);
                            let sub_res = sub
                                .3
                                .get_jsobject()
                                .as_promise_mut()
                                .do_resolve(ctx, sub.3, sub_res);
                            if sub_res.is_err() {
                                println!("could not resolve sub");
                            }
                        }
                    }
                }
                for sub in &prom_self.subs {
                    // invoke 2, resolve 3
                    if let Some(jsFunc) = sub.2 {
                        let this = JsValue::encode_undefined_value();
                        let mut args_vec = vec![];
                        let mut args = Arguments::new(this, args_vec.as_mut_slice());
                        let sub_res = jsFunc
                            .get_jsobject()
                            .as_function_mut()
                            .call(ctx, &mut args, this);
                        let sub_res = sub
                            .3
                            .get_jsobject()
                            .as_promise_mut()
                            .do_resolve(ctx, sub.3, sub_res);
                        if sub_res.is_err() {
                            println!("could not resolve sub");
                        }
                    }
                }
            })?;
            Ok(())
        }
    }
    pub fn then(
        &mut self,
        ctx: GcPointer<Context>,
        on_resolved: Option<JsValue>,
        on_rejected: Option<JsValue>,
        on_finally: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        // add functions to vec with tuples (jsFunc, Prom)

        let sub_prom = Self::new_unresolving(ctx)?;

        self.subs
            .push((on_resolved, on_rejected, on_finally, sub_prom));

        Ok(sub_prom)
    }

    define_jsclass_with_symbol!(
        JsObject,
        Promise,
        Object,
        Some(drop_promise_fn),
        Some(prom_trace),
        Some(deser),
        Some(ser),
        Some(prom_size)
    );
}

fn array_util_get_length(
    ctx: GcPointer<Context>,
    arr_object: &mut GcPointer<JsObject>,
) -> Result<u32, JsValue> {
    get_length(ctx, arr_object)
}

fn array_util_get_value_at(
    ctx: GcPointer<Context>,
    arr_object: &mut GcPointer<JsObject>,
    index: u32,
) -> Result<JsValue, JsValue> {
    arr_object.get(ctx, Symbol::Index(index))
}

fn array_util_set_value_at(
    ctx: GcPointer<Context>,
    arr_object: &mut GcPointer<JsObject>,
    index: u32,
    value: JsValue,
) -> Result<(), JsValue> {
    arr_object.put(ctx, Symbol::Index(index), value, false)
}

extern "C" fn drop_promise_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<JsPromise>()) }
}

#[allow(improper_ctypes_definitions)]
extern "C" fn prom_trace(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<JsPromise>().trace(tracer);
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer) {
    unreachable!("Cannot deserialize a Promise");
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    unreachable!("Cannot serialize a Promise");
}
extern "C" fn prom_size() -> usize {
    std::mem::size_of::<JsPromise>()
}

unsafe impl Trace for JsPromise {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.resolution.trace(tracer);
        self.subs.iter_mut().for_each(|sub| {
            sub.0.trace(tracer);
            sub.1.trace(tracer);
            sub.2.trace(tracer);
            sub.3.trace(tracer);
        });
        if let Some(tracking_results) = &mut self.tracking_results {
            tracking_results.trace(tracer);
        }
    }
}

#[cfg(test)]
pub mod tests {

    use crate::options::Options;
    use crate::vm::context::Context;
    use crate::Platform;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_promise() {
        //Platform::initialize();
        let todos = Rc::new(RefCell::new(vec![]));
        let todos2 = todos.clone();
        let options = Options::default();
        println!("starting");
        let mut starlight_runtime =
            Platform::new_runtime(options, None).with_async_scheduler(Box::new(move |job| {
                println!("sched job");
                todos2.borrow_mut().push(job);
            }));
        let mut ctx = Context::new(&mut starlight_runtime);

        match ctx
            .eval("let p = new Promise((res, rej) => {print('running promise'); res(123);}); p.then((res) => {print('p resolved to ' + res);});")
        {
            Ok(_) => {

                println!("prom code running");
            }
            Err(e) => {
                println!(
                    "prom init failed: {}",
                    e.to_string(ctx)
                        .ok()
                        .expect("conversion failed")
                );
            }
        }

        match ctx
            .eval("let p = new Promise((resa, rejb) => {print('running promise'); resa(new Promise((resb, rejb) => {resb(321);}));}); p.then((res) => {print('p resolved to ' + res);});")
        {
            Ok(_) => {

                println!("prom code running");
            }
            Err(e) => {
                println!(
                    "prom init failed: {}",
                    e.to_string(ctx)
                        .ok()
                        .expect("conversion failed")
                );
            }
        }

        match ctx
            .eval("let p = Promise.all([new Promise((resA, rejA) => {resA(123);}), new Promise((resB, rejB) => {resB(456);})]); p.then((res) => {print('pAll resolved to ' + res.join(','));}); p.catch((res) => {print('pAll rejected to ' + res);});")
        {
            Ok(_) => {

                println!("prom code running");
            }
            Err(e) => {
                println!(
                    "prom init failed: {}",
                    e.to_string(ctx)
                        .ok()
                        .expect("conversion failed")
                );
            }
        }

        loop {
            let job;
            {
                let todos_vec = &mut *todos.borrow_mut();
                if todos_vec.is_empty() {
                    break;
                }
                job = todos_vec.remove(0);
            }

            println!("running todo");

            job(ctx);
        }
        println!("done running todos");
    }
}
