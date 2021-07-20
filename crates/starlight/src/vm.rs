/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use self::{attributes::*, context::Context, object::JsObject, structure::Structure};
use crate::{
    bytecompiler::{ByteCompiler, CompileError},
    gc::default_heap,
    gc::shadowstack::ShadowStack,
    gc::Heap,
    gc::{
        cell::GcPointer,
        cell::Trace,
        cell::{GcCell, GcPointerBase, Tracer},
        SimpleMarkingConstraint,
    },
    gc::{safepoint::GlobalSafepoint, snapshot::Snapshot},
    options::Options,
};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    u32, u8, usize,
};
use std::{fmt::Display, io::Write, sync::RwLock};
use swc_common::{
    errors::{DiagnosticBuilder, Emitter, Handler},
    input::StringInput,
    sync::Lrc,
};
use swc_common::{FileName, SourceMap};
use swc_ecmascript::{
    ast::{ExprOrSpread, Program},
    parser::{error::Error, EsConfig, Parser, Syntax},
};
#[macro_use]
pub mod class;
#[macro_use]
pub mod method_table;
pub mod arguments;
pub mod array;
pub mod array_buffer;
pub mod array_storage;
pub mod attributes;
pub mod bigint;
pub mod builtins;
pub mod code_block;
pub mod context;
pub mod data_view;
pub mod environment;
pub mod error;
pub mod function;
pub mod global;
pub mod indexed_elements;
pub mod interpreter;
pub mod map;
pub mod native_iterator;
pub mod number;
pub mod object;
pub mod operations;
pub mod perf;
pub mod property_descriptor;
pub mod slot;
pub mod string;
pub mod structure;
pub mod structure_builder;
pub mod structure_chain;
pub mod symbol_table;
pub mod thread;
pub mod typedarray;
pub mod value;
use crate::gc::snapshot::{deserializer::*, serializer::*};

use value::*;
pub mod promise;

#[derive(Copy, Clone)]
pub enum ModuleKind {
    Initialized(GcPointer<JsObject>),
    NativeUninit(fn(GcPointer<Context>, GcPointer<JsObject>) -> Result<(), JsValue>),
}
impl GcCell for ModuleKind {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for ModuleKind {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        if let Self::Initialized(x) = self {
            x.trace(visitor)
        }
    }
}

impl Serializable for ModuleKind {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        match self {
            Self::Initialized(x) => {
                serializer.write_u8(0x0);
                x.serialize(serializer);
            }
            Self::NativeUninit(x) => {
                serializer.write_u8(0x1);
                serializer.write_reference((*x) as *const u8);
            }
        }
    }
}

impl Deserializable for ModuleKind {
    unsafe fn allocate(_ctx: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        unreachable!()
    }

    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let byte = deser.get_u8();
        match byte {
            0x0 => ModuleKind::Initialized(GcPointer::<JsObject>::deserialize_inplace(deser)),
            0x1 => ModuleKind::NativeUninit(std::mem::transmute(deser.get_reference())),
            _ => unreachable!(),
        }
    }
    unsafe fn deserialize(_at: *mut u8, _deser: &mut Deserializer) {
        unreachable!()
    }
    unsafe fn dummy_read(_deser: &mut Deserializer) {
        unreachable!()
    }
}

/// JavaScript runtime instance.
pub struct Runtime {
    pub(crate) gc: Heap,
    pub(crate) external_references: Option<&'static [usize]>,
    pub(crate) options: Options,
    pub(crate) shadowstack: ShadowStack,
    pub(crate) codegen_plugins: HashMap<
        String,
        Box<
            dyn Fn(
                &mut ByteCompiler,
                GcPointer<Context>,
                &Vec<ExprOrSpread>,
            ) -> Result<(), CompileError>,
        >,
    >,
    #[cfg(feature = "perf")]
    pub(crate) perf: perf::Perf,
    #[allow(dead_code)]
    /// String that contains all the source code passed to [Runtime::eval] and [Runtime::evalm]
    pub(crate) eval_history: String,
    pub(crate) persistent_roots: Rc<RefCell<HashMap<usize, JsValue>>>,
    pub(crate) sched_async_func: Option<Box<dyn Fn(Box<dyn FnOnce(GcPointer<Context>)>)>>,
    pub(crate) safepoint: GlobalSafepoint,

    pub(crate) contexts: Vec<GcPointer<Context>>,

    pub(crate) context_snapshot: Rc<Box<[u8]>>,
}

impl Runtime {
    /// initialize a Runtime with an async scheduler
    /// the async scheduler is used to asynchronously run jobs with the Runtime
    /// this can be used for things like Promises, setImmediate, async functions
    /// # Example
    /// ```rust
    /// use starlight::Platform;
    /// use starlight::options::Options;
    /// Platform::initialize();
    /// let options = Options::default();
    /// let mut starlight_runtime = Platform::new_runtime(options, None).with_async_scheduler(Box::new(move |job| {
    ///     // here you would add the job to your EventLoop
    ///     // e.g.:
    ///     // EventLoop.add_local_void(move || {
    ///     //     RtThreadLocal.with(|rc| {
    ///     //         let sl_rt = &mut *rc.borrow_mut();
    ///     //         job(rt);
    ///     //     });
    ///     // });
    ///     println!("sched async job...");
    /// }));
    /// ```
    pub fn with_async_scheduler(
        mut self: Box<Self>,
        scheduler: Box<dyn Fn(Box<dyn FnOnce(GcPointer<Context>)>)>,
    ) -> Box<Self> {
        self.sched_async_func = Some(scheduler);
        self
    }
    pub fn add_persistent_root(&mut self, obj: JsValue) -> PersistentRooted {
        // for PoC only, todo use something like AutoIdMap for persistent_roots

        let pr = &mut *self.persistent_roots.borrow_mut();

        let mut id = 0;
        while pr.contains_key(&id) {
            id += 1;
        }
        pr.insert(id, obj);
        PersistentRooted {
            id,
            map: self.persistent_roots.clone(),
        }
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn new_raw(
        gc: Heap,
        options: Options,
        external_references: Option<&'static [usize]>,
    ) -> Self {
        Self {
            gc,
            options,
            safepoint: GlobalSafepoint::new(),
            external_references,
            shadowstack: ShadowStack::new(),
            #[cfg(feature = "perf")]
            perf: perf::Perf::new(),
            eval_history: String::new(),
            persistent_roots: Default::default(),
            sched_async_func: None,
            codegen_plugins: HashMap::new(),
            contexts: vec![],
            context_snapshot: Rc::new(Box::new([])),
        }
    }

    pub fn new(options: Options, external_references: Option<&'static [usize]>) -> Box<Self> {
        Self::with_heap(default_heap(&options), options, external_references)
    }

    /// Get mutable heap reference.
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.gc
    }

    /// Construct runtime instance with specific GC heap.
    pub fn with_heap(
        gc: Heap,
        options: Options,
        external_references: Option<&'static [usize]>,
    ) -> Box<Self> {
        let mut this = Box::new(Runtime::new_raw(gc, options, external_references));
        let vm = &mut *this as *mut Runtime;
        this.gc.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                // rt.shadowstack.trace(visitor);
                rt.contexts.iter_mut().for_each(|ctx| {
                    ctx.trace(visitor);
                });
                let pr = &mut *rt.persistent_roots.borrow_mut();
                pr.iter_mut().for_each(|entry| {
                    entry.1.trace(visitor);
                });
            },
        ));
        this
    }

    pub(crate) fn new_empty(
        gc: Heap,
        options: Options,
        external_references: Option<&'static [usize]>,
    ) -> Box<Self> {
        let mut this = Box::new(Runtime::new_raw(gc, options, external_references));
        let vm = &mut *this as *mut Runtime;
        this.gc.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                // rt.shadowstack.trace(visitor);
                rt.contexts.iter_mut().for_each(|ctx| ctx.trace(visitor));
                let pr = &mut *rt.persistent_roots.borrow_mut();
                pr.iter_mut().for_each(|entry| {
                    entry.1.trace(visitor);
                });
            },
        ));

        this
    }

    pub fn shadowstack<'a>(&self) -> &'a ShadowStack {
        unsafe { std::mem::transmute(&self.shadowstack) }
    }
    /// Enable FFI builtin object.
    ///
    ///
    /// FFI object allows to load arbitrary dynamic library and then load functions from it.
    #[cfg(all(target_pointer_width = "64", feature = "ffi"))]
    pub fn add_ffi(&mut self) {
        crate::jsrt::ffi::initialize_ffi(self);
    }

    pub fn register_codegen_plugin(
        &mut self,
        plugin_name: &str,
        codegen_func: Box<
            dyn Fn(
                &mut ByteCompiler,
                GcPointer<Context>,
                &Vec<ExprOrSpread>,
            ) -> Result<(), CompileError>,
        >,
    ) -> Result<(), &str> {
        if !self.options.codegen_plugins {
            return Err("Need enable codegen_plugins option to register codegen plugin!");
        }
        self.codegen_plugins
            .insert(String::from(plugin_name), codegen_func);
        Ok(())
    }

    pub fn remove_context(&mut self, ctx: GcPointer<Context>) {
        let mut contexts = &mut self.contexts;
        let index = contexts
            .iter_mut()
            .position(|x| *x == ctx)
            .expect("context not found");
        self.contexts.remove(index);
    }

    pub fn context(&mut self, index: usize) -> GcPointer<Context> {
        let ctx = self.contexts.get(index);
        *ctx.unwrap()
    }

    pub fn new_context(&mut self) -> GcPointer<Context> {
        if self.context_snapshot.len() == 0 {
            let ctx = Context::new(self);
            self.context_snapshot =
                Rc::new(Snapshot::take_context(false, self, ctx, |_, _| {}).buffer);
            ctx
        } else {
            let snapshot = self.context_snapshot.clone();
            Deserializer::deserialize_context(self, false, &snapshot)
        }
    }
}

pub struct PersistentRooted {
    id: usize,
    map: Rc<RefCell<HashMap<usize, JsValue>>>,
}

impl PersistentRooted {
    pub fn get_value(&self) -> JsValue {
        *self.map.borrow().get(&self.id).unwrap()
    }
}

impl Drop for PersistentRooted {
    fn drop(&mut self) {
        let map = &mut *self.map.borrow_mut();
        map.remove(&self.id);
    }
}

use starlight_derive::GcTrace;
use wtf_rs::unwrap_unchecked;

use std::cell::RefCell;
use std::rc::Rc;

/// Global JS data that is used internally in Starlight.
#[derive(Default, GcTrace)]
pub struct GlobalData {
    pub(crate) generator_prototype: Option<GcPointer<JsObject>>,
    pub(crate) generator_structure: Option<GcPointer<Structure>>,
    pub(crate) normal_arguments_structure: Option<GcPointer<Structure>>,
    pub(crate) empty_object_struct: Option<GcPointer<Structure>>,
    pub(crate) function_struct: Option<GcPointer<Structure>>,
    pub(crate) object_prototype: Option<GcPointer<JsObject>>,
    pub(crate) object_constructor: Option<GcPointer<JsObject>>,
    pub(crate) number_prototype: Option<GcPointer<JsObject>>,
    pub(crate) string_prototype: Option<GcPointer<JsObject>>,
    pub(crate) boolean_prototype: Option<GcPointer<JsObject>>,
    pub(crate) symbol_prototype: Option<GcPointer<JsObject>>,
    pub(crate) error: Option<GcPointer<JsObject>>,
    pub(crate) type_error: Option<GcPointer<JsObject>>,
    pub(crate) reference_error: Option<GcPointer<JsObject>>,
    pub(crate) range_error: Option<GcPointer<JsObject>>,
    pub(crate) syntax_error: Option<GcPointer<JsObject>>,
    pub(crate) internal_error: Option<GcPointer<JsObject>>,
    pub(crate) eval_error: Option<GcPointer<JsObject>>,
    pub(crate) array_prototype: Option<GcPointer<JsObject>>,
    pub(crate) func_prototype: Option<GcPointer<JsObject>>,
    pub(crate) string_structure: Option<GcPointer<Structure>>,
    pub(crate) number_structure: Option<GcPointer<Structure>>,
    pub(crate) array_structure: Option<GcPointer<Structure>>,
    pub(crate) error_structure: Option<GcPointer<Structure>>,
    pub(crate) range_error_structure: Option<GcPointer<Structure>>,
    pub(crate) reference_error_structure: Option<GcPointer<Structure>>,
    pub(crate) syntax_error_structure: Option<GcPointer<Structure>>,
    pub(crate) type_error_structure: Option<GcPointer<Structure>>,
    pub(crate) uri_error_structure: Option<GcPointer<Structure>>,
    pub(crate) eval_error_structure: Option<GcPointer<Structure>>,
    pub(crate) map_structure: Option<GcPointer<Structure>>,
    pub(crate) set_structure: Option<GcPointer<Structure>>,
    pub(crate) map_prototype: Option<GcPointer<JsObject>>,
    pub(crate) set_prototype: Option<GcPointer<JsObject>>,
    pub(crate) regexp_structure: Option<GcPointer<Structure>>,
    pub(crate) regexp_prototype: Option<GcPointer<JsObject>>,
    pub(crate) array_buffer_prototype: Option<GcPointer<JsObject>>,
    pub(crate) array_buffer_structure: Option<GcPointer<Structure>>,
    pub(crate) data_view_structure: Option<GcPointer<Structure>>,
    pub(crate) data_view_prototype: Option<GcPointer<JsObject>>,
    pub(crate) spread_builtin: Option<GcPointer<JsObject>>,
    pub(crate) weak_ref_structure: Option<GcPointer<Structure>>,
    pub(crate) weak_ref_prototype: Option<GcPointer<JsObject>>,
    pub(crate) symbol_structure: Option<GcPointer<Structure>>,
    pub(crate) date_structure: Option<GcPointer<Structure>>,
    pub(crate) date_prototype: Option<GcPointer<JsObject>>,
    pub(crate) boolean_structure: Option<GcPointer<Structure>>,
}

impl GlobalData {
    pub fn get_function_struct(&self) -> GcPointer<Structure> {
        unwrap_unchecked(self.function_struct)
    }

    pub fn get_object_prototype(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.object_prototype)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct RuntimeRef(pub(crate) *mut Runtime);

impl Deref for RuntimeRef {
    type Target = Runtime;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl DerefMut for RuntimeRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}
#[derive(Clone, Default)]
pub(crate) struct BufferedError(std::sync::Arc<RwLock<String>>);

impl Write for BufferedError {
    fn write(&mut self, d: &[u8]) -> std::io::Result<usize> {
        self.0
            .write()
            .unwrap()
            .push_str(&String::from_utf8_lossy(d));

        Ok(d.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Display for BufferedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.0.read().unwrap(), f)
    }
}
#[derive(Clone, Default)]
pub struct MyEmiter(BufferedError);
impl Emitter for MyEmiter {
    fn emit(&mut self, db: &DiagnosticBuilder<'_>) {
        let z = &(self.0).0;
        for msg in &db.message {
            z.write().unwrap().push_str(&msg.0);
        }
    }
}
pub struct OutBuf;

impl std::fmt::Write for OutBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        print!("{}", s);
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        #[cfg(feature = "perf")]
        {
            self.perf.print_perf();
        }
    }
}

pub fn parse(script: &str, strict_mode: bool) -> Result<Program, Error> {
    let cm: Lrc<SourceMap> = Default::default();
    let _e = BufferedError::default();

    let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));
    let script = if strict_mode {
        format!("\"use strict\";\n{}", script)
    } else {
        script.to_string()
    };
    let fm = cm.new_source_file(FileName::Custom("<script>".into()), script);

    let mut parser = Parser::new(Syntax::Es(init_es_config()), StringInput::from(&*fm), None);

    for e in parser.take_errors() {
        e.into_diagnostic(&handler).emit();
    }

    let script = match parser.parse_program() {
        Ok(script) => script,
        Err(e) => {
            return Err(e);
        }
    };

    Ok(script)
}

pub(crate) fn init_es_config() -> EsConfig {
    let mut es_config: EsConfig = Default::default();
    es_config.dynamic_import = true;
    es_config
}

#[cfg(test)]
pub mod tests {
    use crate::gc::cell::GcPointer;
    use crate::options::Options;
    use crate::vm::symbol_table::Internable;
    use crate::vm::value::JsValue;
    use crate::vm::{arguments, context::Context, Runtime};
    use crate::Platform;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_simple_async() {
        // start a runtime
        Platform::initialize();

        type JobType = dyn FnOnce(GcPointer<Context>);
        // the todo Rc is where we will store our async job, in real life this would be an EventLoop to which we could add multiple jobs
        let todo: Rc<RefCell<Option<Box<JobType>>>> = Rc::new(RefCell::new(None));
        let todo_clone = todo.clone();

        // in real life you would add this job to an EventLoop
        let options = Options::default();

        let mut starlight_runtime =
            Platform::new_runtime(options, None).with_async_scheduler(Box::new(move |job| {
                println!("sched async job...");
                let opt = &mut *todo_clone.borrow_mut();
                opt.replace(Box::new(job));
            }));
        let mut ctx = Context::new(&mut starlight_runtime);

        let mut global = ctx.global_object();

        // create a symbol for the functions name
        let name_symbol = "setImmediate".intern();

        // create a setImmediate Function
        // the setImmediate function is not part of the official ECMAspec but is implemented in older IE versions
        // it should work about the same as setTimeout(func, 0)
        // see also https://developer.mozilla.org/en-US/docs/Web/API/Window/setImmediate
        // it should serve pretty good as a simple as it gets first example of doing things async
        let arg_count = 0;
        let func = crate::vm::function::JsNativeFunction::new(
            ctx,
            name_symbol,
            move |vm, args| {
                if args.size() == 1 {
                    let func_val = args.at(0);
                    if func_val.is_callable() {
                        match vm.schedule_async(move |vm2| {
                            println!("invoking func_val");
                            // invoke func val here with vm2
                            let mut obj = func_val.get_jsobject();
                            let func = obj.as_function_mut();
                            let this = JsValue::encode_null_value();
                            let mut arguments = arguments::Arguments::new(this, &mut []);
                            let res = func.call(vm2, &mut arguments, this);
                            match res {
                                Ok(_) => {
                                    println!("job exe ok");
                                }
                                Err(e) => {
                                    panic!(
                                        "job exe fail: {}",
                                        e.to_string(vm2).ok().expect("conversion failed")
                                    );
                                }
                            }
                        }) {
                            Ok(_) => Ok(JsValue::encode_null_value()),
                            Err(_err) => {
                                // todo encode str
                                Err(JsValue::encode_null_value())
                            }
                        }
                    } else {
                        // "args was not callable"
                        // todo encode str
                        Err(JsValue::encode_null_value())
                    }
                } else {
                    // todo return string value
                    // "need one arg"
                    Err(JsValue::encode_null_value())
                }
            },
            arg_count,
        );

        // add the function to the global object
        global
            .put(ctx, name_symbol, JsValue::new(func), true)
            .ok()
            .expect("could not add func to global");

        // run the function
        let _outcome = match ctx.eval("setImmediate(() => {print('later')}); print('first');") {
            Ok(e) => e,
            Err(err) => panic!(
                "func failed: {}",
                err.to_string(ctx).ok().expect("conversion failed")
            ),
        };

        if let Some(job) = todo.take() {
            job(ctx);
        } else {
            panic!("did not get job")
        }
    }

    use swc_ecmascript::ast::ExprOrSpread;

    use crate::{bytecode::opcodes::Opcode, bytecompiler::ByteCompiler, gc::default_heap};

    #[test]
    fn test_register_codegen_plugin() {
        Platform::initialize();
        let options: Options = Options::default().with_codegen_plugins(true);
        let heap = default_heap(&options);
        let mut rt = Runtime::with_heap(heap, options, None);
        let mut ctx = Context::new(&mut rt);

        let result = rt.register_codegen_plugin(
            "MyOwnAddFn",
            Box::new(
                |compiler: &mut ByteCompiler,
                 ctx: GcPointer<Context>,
                 call_args: &Vec<ExprOrSpread>| {
                    compiler.expr(ctx, &call_args[0].expr, true, false)?;
                    compiler.expr(ctx, &call_args[1].expr, true, false)?;
                    compiler.emit(Opcode::OP_ADD, &[0], false);
                    Ok(())
                },
            ),
        );
        assert!(result.is_ok(), "Should register success!");
        let result = ctx.eval("MyOwnAddFn(2,3)");
        assert!(result.is_ok(), "Should get result");
        if let Ok(value) = result {
            assert_eq!(5, value.get_int32());
        }

        Platform::initialize();
        let options: Options = Options::default();
        let heap = default_heap(&options);
        let mut rt = Runtime::with_heap(heap, options, None);
        let mut ctx = Context::new(&mut rt);

        let result = rt.register_codegen_plugin(
            "MyOwnAddFn",
            Box::new(
                |compiler: &mut ByteCompiler,
                 ctx: GcPointer<Context>,
                 call_args: &Vec<ExprOrSpread>| {
                    compiler.expr(ctx, &call_args[0].expr, true, false)?;
                    compiler.expr(ctx, &call_args[1].expr, true, false)?;
                    compiler.emit(Opcode::OP_ADD, &[0], false);
                    Ok(())
                },
            ),
        );
        assert!(result.is_err(), "Should can't register codegen plugin!");
        let result = ctx.eval("MyOwnAddFn(2,3)");
        assert!(result.is_err(), "Should return JsValue error");
        //
    }
}
