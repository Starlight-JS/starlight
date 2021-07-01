use super::arguments::*;
use super::method_table::*;
use super::object::*;
use super::string::*;
use super::symbol_table::Internable;
use super::value::*;
use super::Runtime;
use crate::gc::cell::{Trace, Tracer};
use crate::gc::snapshot::deserializer::Deserializer;
use crate::gc::snapshot::serializer::SnapshotSerializer;
use crate::vm::function::JsClosureFunction;
use crate::vm::structure::Structure;
use std::mem::ManuallyDrop;

pub struct JsPromise {
    subs: Vec<(Option<JsValue>, Option<JsValue>, Option<JsValue>, JsValue)>, // then_func / catch_func / finally_func / sub_promise
    resolution: Option<Result<JsValue, JsValue>>,
}

#[allow(non_snake_case)]
impl JsPromise {
    pub fn new(vm: &mut Runtime, function_value: JsValue) -> Result<JsValue, JsValue> {
        let promise = Self::new_unresolving(vm)?;

        // add persistentrooted for promise and function_value here..
        let prom_rooted_id = vm.add_persistent_root(promise);
        let func_rooted_id = vm.add_persistent_root(function_value);

        let sched_res = vm.schedule_async(move |vm| {
            // here we are running async
            // call the function passed to the promise constructor with a resolve and a reject arg


            let resolve_func = JsValue::encode_object_value(JsClosureFunction::new(
                vm,
                "resolve".intern(),
                move |vm, arguments| {
                    let mut promise = promise;
                    // resolve promise here

                    // todo check one arg
                    let resolution = arguments.at(0);
                    promise
                        .get_jsobject()
                        .as_promise_mut()
                        .resolve(vm, resolution)?;
                    Ok(JsValue::encode_undefined_value())
                },
                1,
            ));
            let reject_func = JsValue::encode_object_value(JsClosureFunction::new(
                vm,
                "reject".intern(),
                move |vm, arguments| {
                    let mut promise = promise;
                    // reject promise here

                    // todo check one arg
                    let rejection = arguments.at(0);
                    promise
                        .get_jsobject()
                        .as_promise_mut()
                        .reject(vm, rejection)?;
                    Ok(JsValue::encode_undefined_value())
                },
                1,
            ));

            let mut args_vec = vec![resolve_func, reject_func];
            let mut arguments =
                Arguments::new(JsValue::encode_undefined_value(), args_vec.as_mut_slice());

            let res = function_value.get_jsobject().as_function_mut().call(
                vm,
                &mut arguments,
                JsValue::encode_undefined_value(),
            );

            match res {
                Ok(_) => {}
                Err(err) => {
                    // should this reject the prom?
                    println!(
                        "prom func invoc failed: {}",
                        err.to_string(vm).ok().expect("conversion failed")
                    )
                }
            }

            vm.remove_persistent_root(&prom_rooted_id);
            vm.remove_persistent_root(&func_rooted_id);
        });

        sched_res.map(|_| promise)
    }
    pub fn new_unresolving(vm: &mut Runtime) -> Result<JsValue, JsValue> {
        let proto = vm
            .global_object()
            .get(vm, "Promise".intern())?
            .to_object(vm)?
            .get(vm, "prototype".intern())?
            .to_object(vm)?;

        let structure = Structure::new_indexed(vm, Some(proto), false);
        let mut obj = JsObject::new(vm, &structure, JsPromise::get_class(), ObjectTag::Ordinary);

        *obj.data::<JsPromise>() = ManuallyDrop::new(JsPromise {
            subs: vec![],
            resolution: None,
        });
        Ok(JsValue::new(obj))
    }
    pub fn resolve(&mut self, vm: &mut Runtime, resolution: JsValue) -> Result<(), JsValue> {
        self.do_resolve(vm, Ok(resolution))
    }
    pub fn reject(&mut self, vm: &mut Runtime, rejection: JsValue) -> Result<(), JsValue> {
        self.do_resolve(vm, Err(rejection))
    }
    fn do_resolve(
        &mut self,
        vm: &mut Runtime,
        resolution: Result<JsValue, JsValue>,
    ) -> Result<(), JsValue> {
        //println!("do_resolve, subs={}", self.subs.len());

        if self.resolution.is_some() {
            Err(JsValue::encode_object_value(JsString::new(
                vm,
                "Promise was already resolved",
            )))
        } else {
            self.resolution = Some(resolution);

            // todo everything below needs to be in async job.. need to root persistent again... later

            if let Some(ok_resolution) = self.resolution.unwrap().ok() {
                for sub in &self.subs {
                    // invoke 0, resolve 3
                    if let Some(jsFunc) = sub.0 {
                        let this = JsValue::encode_undefined_value();
                        let mut args_vec = vec![ok_resolution];
                        let mut args = Arguments::new(this, args_vec.as_mut_slice());
                        let sub_res = jsFunc
                            .get_jsobject()
                            .as_function_mut()
                            .call(vm, &mut args, this);
                        sub.3
                            .get_jsobject()
                            .as_promise_mut()
                            .do_resolve(vm, sub_res)?
                    }
                }
            } else {
                let err_resolution = self.resolution.unwrap().err().unwrap();
                for sub in &self.subs {
                    // invoke 1, resolve 3
                    if let Some(jsFunc) = sub.1 {
                        let this = JsValue::encode_undefined_value();
                        let mut args_vec = vec![err_resolution];
                        let mut args = Arguments::new(this, args_vec.as_mut_slice());
                        let sub_res = jsFunc
                            .get_jsobject()
                            .as_function_mut()
                            .call(vm, &mut args, this);
                        sub.3
                            .get_jsobject()
                            .as_promise_mut()
                            .do_resolve(vm, sub_res)?
                    }
                }
            }
            for sub in &self.subs {
                // invoke 2, resolve 3
                if let Some(jsFunc) = sub.2 {
                    let this = JsValue::encode_undefined_value();
                    let mut args_vec = vec![];
                    let mut args = Arguments::new(this, args_vec.as_mut_slice());
                    let sub_res = jsFunc
                        .get_jsobject()
                        .as_function_mut()
                        .call(vm, &mut args, this);
                    sub.3
                        .get_jsobject()
                        .as_promise_mut()
                        .do_resolve(vm, sub_res)?
                }
            }
            Ok(())
        }
    }
    pub fn then(
        &mut self,
        vm: &mut Runtime,
        on_resolved: Option<JsValue>,
        on_rejected: Option<JsValue>,
        on_finally: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        // add functions to vec with tuples (jsFunc, Prom)

        let sub_prom = Self::new_unresolving(vm)?;

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

extern "C" fn drop_promise_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<JsPromise>()) }
}

#[allow(improper_ctypes_definitions)]
extern "C" fn prom_trace(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<JsPromise>().trace(tracer);
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer, _: &mut Runtime) {
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
    }
}

#[cfg(test)]
pub mod tests {

    use crate::options::Options;
    use crate::Platform;
    use backtrace::Backtrace;
    use std::cell::RefCell;
    use std::panic;
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

        match starlight_runtime
            .eval("let p = new Promise((res, rej) => {print('running promise'); res(123);}); p.then((res) => {print('p resolved to ' + res);});")
        {
            Ok(_) => {

                println!("prom code running");
            }
            Err(e) => {
                println!(
                    "prom init failed: {}",
                    e.to_string(&mut starlight_runtime)
                        .ok()
                        .expect("conversion failed")
                );
            }
        }

        let todos_vec = &mut *todos.borrow_mut();
        println!("running todos");
        while !todos_vec.is_empty() {
            let job = todos_vec.remove(0);
            job(&mut starlight_runtime);
        }
        println!("done running todos");
    }
}
