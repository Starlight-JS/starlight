/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    bytecompiler::ByteCompiler,
    gc::default_heap,
    gc::shadowstack::ShadowStack,
    gc::Heap,
    gc::{
        cell::GcPointer,
        cell::Trace,
        cell::{GcCell, GcPointerBase, Tracer},
        SimpleMarkingConstraint,
    },
    jsrt::{self, object::*, regexp::RegExp},
    options::Options,
};
use arguments::Arguments;
use environment::Environment;
use error::JsSyntaxError;
use function::JsVMFunction;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use std::{fmt::Display, io::Write, sync::RwLock};
use string::JsString;
use swc_common::{
    errors::{DiagnosticBuilder, Emitter, Handler},
    sync::Lrc,
};
use swc_common::{FileName, SourceMap};
use swc_ecmascript::{
    ast::Program,
    parser::{error::Error, *},
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
pub mod structure_chain;
pub mod symbol_table;
pub mod thread;
pub mod typedarray;
pub mod value;
use crate::gc::snapshot::{deserializer::*, serializer::*};
use attributes::*;
use object::*;
use property_descriptor::*;
use value::*;
pub mod promise;

#[derive(Copy, Clone)]
pub enum ModuleKind {
    Initialized(GcPointer<JsObject>),
    NativeUninit(fn(&mut Runtime, GcPointer<JsObject>) -> Result<(), JsValue>),
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
    unsafe fn allocate(_rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
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
    pub(crate) stack: Stack,
    pub(crate) global_data: GlobalData,
    pub(crate) global_object: Option<GcPointer<JsObject>>,
    pub(crate) external_references: Option<&'static [usize]>,
    pub(crate) options: Options,
    pub(crate) shadowstack: ShadowStack,
    pub(crate) stacktrace: String,
    pub(crate) symbol_table: HashMap<Symbol, GcPointer<JsSymbol>>,
    pub(crate) module_loader: Option<GcPointer<JsObject>>,
    pub(crate) modules: HashMap<String, ModuleKind>,
    #[cfg(feature = "perf")]
    pub(crate) perf: perf::Perf,
    #[allow(dead_code)]
    /// String that contains all the source code passed to [Runtime::eval] and [Runtime::evalm]
    pub(crate) eval_history: String,
    persistent_roots: HashMap<usize, JsValue>,
    sched_async_func: Option<Box<dyn Fn(Box<dyn FnOnce(&mut Runtime)>)>>,
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
    /// let options = Options::default().with_async_scheduler(Box::new(move |job| {
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
    /// let mut starlight_runtime = Platform::new_runtime(options, None);
    /// ```
    pub fn with_async_scheduler(
        mut self,
        scheduler: Box<dyn Fn(Box<dyn FnOnce(&mut Runtime)>)>,
    ) -> Self {
        self.sched_async_func = Some(scheduler);
        self
    }
    pub fn add_persistent_root(&mut self, obj: JsValue) -> usize {
        // for PoC only, todo use something like AutoIdMap for persistent_roots

        let mut id = 0;
        while self.persistent_roots.contains_key(&id) {
            id += 1;
        }
        self.persistent_roots.insert(id, obj);
        id
    }

    pub fn remove_persistent_root(&mut self, id: &usize) {
        self.persistent_roots.remove(id);
    }
    pub(crate) fn schedule_async<F>(&mut self, job: F) -> Result<(), JsValue>
        where
            F: FnOnce(&mut Runtime) + 'static,
    {
        if let Some(scheduler) = &self.sched_async_func {
            scheduler(Box::new(job));
            Ok(())
        } else {
            Err(JsValue::encode_object_value(JsString::new(self, "In order to use async you have to init the RuntimeOptions with with_async_scheduler()")))
        }
    }
    pub fn options(&self) -> &Options {
        &self.options
    }
    /// Find call frame that has try catch block in it. (Does not clean the stack!)
    pub(crate) unsafe fn unwind(&mut self) -> Option<*mut CallFrame> {
        let mut frame = self.stack.current;
        while !frame.is_null() {
            if !(*frame).try_stack.is_empty() {
                return Some(frame);
            }
            let p = self.stack.pop_frame().unwrap();
            // if `exit_on_return` is true then this frame was created from native code.
            if p.exit_on_return {
                break;
            }
            frame = self.stack.current;
        }
        None
    }

    /// Collect stacktrace.
    pub fn stacktrace(&mut self) -> String {
        let mut result = String::new();
        let mut frame = self.stack.current;
        unsafe {
            while !frame.is_null() {
                if let Some(cb) = (*frame).code_block {
                    let name = self.description(cb.name);
                    result.push_str(&format!("  at '{}':'{}'\n", cb.file_name, name));
                } else {
                    result.push_str(" at '<native code>\n");
                }
                frame = (*frame).prev;
            }
        }
        result
    }
    /// Compile provided script into JS function. If error when compiling happens `SyntaxError` instance
    /// is returned.
    pub fn compile(
        &mut self,
        path: &str,
        name: &str,
        script: &str,
        builtins: bool,
    ) -> Result<JsValue, JsValue> {
        let cm: Lrc<SourceMap> = Default::default();
        let _e = BufferedError::default();

        let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));

        let fm = cm.new_source_file(FileName::Custom(name.into()), script.into());

        let mut parser = Parser::new(
            Syntax::Es(Default::default()),
            StringInput::from(&*fm),
            None,
        );

        for e in parser.take_errors() {
            e.into_diagnostic(&handler).emit();
        }

        let script = match parser.parse_script() {
            Ok(script) => script,
            Err(e) => {
                let msg = JsString::new(self, e.kind().msg());
                return Err(JsValue::encode_object_value(JsSyntaxError::new(
                    self, msg, None,
                )));
            }
        };
        let mut vmref = RuntimeRef(self);

        let mut code = ByteCompiler::compile_script(
            &mut *vmref,
            &script,
            &std::path::Path::new(&path)
                .canonicalize()
                .unwrap()
                .parent()
                .map(|x| x.to_str().unwrap().to_string())
                .unwrap_or_else(|| "".to_string()),
            path.to_owned(),
            builtins,
        )?;
        code.name = name.intern();
        //code.display_to(&mut OutBuf).unwrap();

        let env = Environment::new(self, 0);
        let fun = JsVMFunction::new(self, code, env);
        Ok(JsValue::encode_object_value(fun))
    }
    pub fn compile_module(
        &mut self,
        path: &str,
        name: &str,
        script: &str,
    ) -> Result<JsValue, JsValue> {
        let cm: Lrc<SourceMap> = Default::default();
        let _e = BufferedError::default();

        let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));

        let fm = cm.new_source_file(FileName::Custom(name.into()), script.into());

        let mut parser = Parser::new(Syntax::Es(init_es_config()), StringInput::from(&*fm), None);

        for e in parser.take_errors() {
            e.into_diagnostic(&handler).emit();
        }

        let module = match parser.parse_module() {
            Ok(module) => module,
            Err(e) => {
                let msg = JsString::new(self, e.kind().msg());
                return Err(JsValue::encode_object_value(JsSyntaxError::new(
                    self, msg, None,
                )));
            }
        };
        let mut vmref = RuntimeRef(self);

        let mut code = ByteCompiler::compile_module(
            &mut *vmref,
            path,
            &std::path::Path::new(&path)
                .canonicalize()
                .unwrap()
                .parent()
                .map(|x| x.to_str().unwrap().to_string())
                .unwrap_or_else(|| "".to_string()),
            name,
            &module,
        )?;
        code.name = name.intern();

        let env = Environment::new(self, 0);
        let fun = JsVMFunction::new(self, code, env);
        Ok(JsValue::encode_object_value(fun))
    }
    /// Evaluates provided script.
    pub fn eval(&mut self, script: &str) -> Result<JsValue, JsValue> {
        self.eval_internal(None, false, script, false)
    }
    /// Tries to evaluate provided `script`. If error when parsing or execution occurs then `Err` with exception value is returned.
    ///
    ///
    ///
    /// TODO: Return script execution result. Right now just `undefined` value is returned.
    pub fn eval_internal(
        &mut self,
        path: Option<&str>,
        force_strict: bool,
        script: &str,
        builtins: bool,
    ) -> Result<JsValue, JsValue> {
        let res = {
            let cm: Lrc<SourceMap> = Default::default();
            let _e = BufferedError::default();

            let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));

            let fm = cm.new_source_file(FileName::Custom("<script>".into()), script.into());

            let mut parser =
                Parser::new(Syntax::Es(init_es_config()), StringInput::from(&*fm), None);

            for e in parser.take_errors() {
                e.into_diagnostic(&handler).emit();
            }

            let script = match parser.parse_script() {
                Ok(script) => script,
                Err(e) => {
                    let msg = JsString::new(self, e.kind().msg());
                    return Err(JsValue::encode_object_value(JsSyntaxError::new(
                        self, msg, None,
                    )));
                }
            };
            let mut vmref = RuntimeRef(self);
            let mut code = ByteCompiler::compile_eval(
                &mut *vmref,
                &script,
                &path
                    .map(|path| match std::path::Path::new(&path).canonicalize() {
                        Ok(x) => x
                            .parent()
                            .map(|x| x.to_str().unwrap().to_string())
                            .unwrap_or_else(|| "".to_string()),
                        Err(_) => String::new(),
                    })
                    .unwrap_or_else(|| "".to_string()),
                path.map(|x| x.to_owned()).unwrap_or_else(String::new),
                builtins,
            )?;
            code.strict = code.strict || force_strict;
            // code.file_name = path.map(|x| x.to_owned()).unwrap_or_else(|| String::new());
            //code.display_to(&mut OutBuf).unwrap();
            let stack = self.shadowstack();

            letroot!(env = stack, Environment::new(self, 0));
            letroot!(fun = stack, JsVMFunction::new(self, code, *env));
            letroot!(func = stack, *fun);
            letroot!(
                args = stack,
                Arguments::new(JsValue::encode_undefined_value(), &mut [])
            );

            fun.as_function_mut()
                .call(self, &mut args, JsValue::new(*func))
        };
        res
    }
    pub fn evalm(
        &mut self,
        path: Option<&str>,
        force_strict: bool,
        script: &str,
    ) -> Result<JsValue, JsValue> {
        let res = {
            let cm: Lrc<SourceMap> = Default::default();
            let _e = BufferedError::default();

            let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));

            let fm = cm.new_source_file(FileName::Custom("<script>".into()), script.into());

            let mut parser =
                Parser::new(Syntax::Es(init_es_config()), StringInput::from(&*fm), None);

            for e in parser.take_errors() {
                e.into_diagnostic(&handler).emit();
            }

            let script = match parser.parse_module() {
                Ok(script) => script,
                Err(e) => {
                    let msg = JsString::new(self, e.kind().msg());
                    return Err(JsValue::encode_object_value(JsSyntaxError::new(
                        self, msg, None,
                    )));
                }
            };
            let mut vmref = RuntimeRef(self);
            let mut code = ByteCompiler::compile_module(
                &mut *vmref,
                &path.map(|x| x.to_owned()).unwrap_or_else(String::new),
                &path
                    .map(|path| {
                        std::path::Path::new(&path)
                            .canonicalize()
                            .unwrap()
                            .parent()
                            .map(|x| x.to_str().unwrap().to_string())
                            .unwrap_or_else(|| "".to_string())
                    })
                    .unwrap_or_else(|| "".to_string()),
                &path.map(|x| x.to_owned()).unwrap_or_else(String::new),
                &script,
            )?;
            code.strict = code.strict || force_strict;

            let stack = self.shadowstack();

            letroot!(env = stack, Environment::new(self, 0));
            letroot!(fun = stack, JsVMFunction::new(self, code, *env));
            letroot!(func = stack, *fun);
            letroot!(module_object = stack, JsObject::new_empty(self));
            let exports = JsObject::new_empty(self);
            module_object
                .put(self, "@exports".intern(), JsValue::new(exports), false)
                .unwrap_or_else(|_| unreachable!());
            let mut args = [JsValue::new(*module_object)];
            letroot!(
                args = stack,
                Arguments::new(
                    JsValue::encode_object_value(self.global_object()),
                    &mut args
                )
            );

            fun.as_function_mut()
                .call(self, &mut args, JsValue::new(*func))
        };
        res
    }
    /// Get global variable, on error returns `None`
    pub fn get_global(&mut self, name: impl AsRef<str>) -> Option<JsValue> {
        match self.global_object().get(self, name.as_ref().intern()) {
            Ok(var) => Some(var),
            Err(_) => None,
        }
    }
    /// Return [Symbol](crate::vm::symbol_table::Symbol) description.
    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(key) | Symbol::Private(key) => {
                symbol_table::symbol_table().description(key).to_owned()
            }
            Symbol::Index(x) => x.to_string(),
        }
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
        let mut this = Box::new(Self {
            gc,
            options,
            stack: Stack::new(),
            modules: HashMap::new(),
            global_object: None,
            stacktrace: String::new(),
            global_data: GlobalData::default(),
            external_references,
            shadowstack: ShadowStack::new(),
            #[cfg(feature = "perf")]
            perf: perf::Perf::new(),
            module_loader: None,
            symbol_table: HashMap::new(),
            eval_history: String::new(),
            persistent_roots: Default::default(),
            sched_async_func: None
        });
        let vm = &mut *this as *mut Runtime;
        this.gc.defer();
        this.gc.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
                rt.stack.trace(visitor);
                rt.shadowstack.trace(visitor);
                rt.module_loader.trace(visitor);
                rt.modules.trace(visitor);
            },
        ));
        this.global_data.empty_object_struct = Some(Structure::new_indexed(&mut this, None, false));
        let s = Structure::new_unique_indexed(&mut this, None, false);
        let mut proto = JsObject::new(&mut this, &s, JsObject::get_class(), ObjectTag::Ordinary);
        this.global_data.object_prototype = Some(proto);
        this.global_data.function_struct = Some(Structure::new_indexed(&mut this, None, false));
        this.global_data.normal_arguments_structure =
            Some(Structure::new_indexed(&mut this, None, false));
        this.global_object = Some(JsGlobal::new(&mut this));
        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto);

        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto);
        this.global_data.number_structure = Some(Structure::new_indexed(&mut this, None, false));
        this.init_func(proto);
        this.init_error(proto);
        this.init_array(proto);
        this.init_promise().ok().expect("init prom failed");
        this.init_math();
        crate::jsrt::number::init_number(&mut this, proto);
        this.init_builtin();

        jsrt::symbol::symbol_init(&mut this, proto);

        let name = "Object".intern();
        let mut obj_constructor = JsNativeFunction::new(&mut this, name, object_constructor, 1);
        super::jsrt::object_init(&mut this, obj_constructor, proto);

        let _ = this.global_object().define_own_property(
            &mut this,
            name,
            &*DataDescriptor::new(JsValue::from(obj_constructor), W | C),
            false,
        );
        let global = this.global_object();

        let _name = "Object".intern();
        let _ = this.global_object().put(
            &mut this,
            "globalThis".intern(),
            JsValue::encode_object_value(global),
            false,
        );
        RegExp::init(&mut this, proto);
        jsrt::generator::init_generator(&mut this, proto);
        this.init_self_hosted();

        let loader = JsNativeFunction::new(&mut this, "@loader".intern(), jsrt::module_load, 1);
        this.module_loader = Some(loader);
        this.add_module(
            "std",
            ModuleKind::NativeUninit(crate::jsrt::jsstd::init_js_std),
        )
        .unwrap();
        assert!(this.modules.contains_key("std"));
        jsrt::array_buffer::array_buffer_init(&mut this);
        this.gc.undefer();
        this.gc.collect_if_necessary();
        this
    }
    /// Get stacktrace. If there was no error then returned string is empty.
    pub fn take_stacktrace(&mut self) -> String {
        std::mem::take(&mut self.stacktrace)
    }
    pub(crate) fn new_empty(
        gc: Heap,
        options: Options,
        external_references: Option<&'static [usize]>,
    ) -> Box<Self> {
        let mut this = Box::new(Self {
            gc,
            options,
            modules: HashMap::new(),
            stack: Stack::new(),
            global_object: None,
            eval_history: String::new(),
            global_data: GlobalData::default(),
            external_references,
            stacktrace: String::new(),
            shadowstack: ShadowStack::new(),
            #[cfg(feature = "perf")]
            perf: perf::Perf::new(),
            symbol_table: HashMap::new(),
            module_loader: None,
            persistent_roots: Default::default(),
            sched_async_func: None
        });
        let vm = &mut *this as *mut Runtime;
        this.gc.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
                rt.stack.trace(visitor);
                rt.shadowstack.trace(visitor);
                rt.module_loader.trace(visitor);
                rt.modules.trace(visitor);
                rt.persistent_roots.iter_mut().for_each(|entry| {
                    entry.1.trace(visitor);
                });
            },
        ));

        this
    }
    /// Create new JS runtime with `MiGC` set as GC.
    pub fn new(options: Options, external_references: Option<&'static [usize]>) -> Box<Runtime> {
        Self::with_heap(default_heap(&options), options, external_references)
    }

    /// Obtain global object reference.
    pub fn global_object(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.global_object)
    }
    pub fn add_module(
        &mut self,
        name: &str,
        mut module_object: ModuleKind,
    ) -> Result<Option<ModuleKind>, &str> {
        if let ModuleKind::Initialized(ref mut module_object) = module_object {
            if !module_object.has_own_property(self, "@exports".intern()) {
                return Err("module does not contain '@exports' property");
            }
        }

        Ok(self.modules.insert(name.to_string(), module_object))
    }
    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
    /// Return "global" shadow stack instance. Note that returned instance is bound to
    /// scope where this function was invoked.
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

    /// Construct new type error from provided string.
    pub fn new_type_error(&mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsTypeError::new(self, msg, None)
    }
    /// Construct new reference error from provided string.
    pub fn new_reference_error(&mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsReferenceError::new(self, msg, None)
    }
    /// Construct new syntax error from provided string.
    pub fn new_syntax_error(&mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsSyntaxError::new(self, msg, None)
    }
}

use starlight_derive::GcTrace;
use wtf_rs::unwrap_unchecked;

use self::{
    error::{JsReferenceError, JsTypeError},
    function::JsNativeFunction,
    global::JsGlobal,
    interpreter::{frame::CallFrame, stack::Stack},
    object::JsObject,
    structure::Structure,
    symbol_table::{Internable, JsSymbol, Symbol},
};

/// Global JS data that is used internally in Starlight.
#[derive(Default, GcTrace)]
pub struct GlobalData {
    pub(crate) generator_prototype: Option<GcPointer<JsObject>>,
    pub(crate) generator_structure: Option<GcPointer<Structure>>,
    pub(crate) normal_arguments_structure: Option<GcPointer<Structure>>,
    pub(crate) empty_object_struct: Option<GcPointer<Structure>>,
    pub(crate) function_struct: Option<GcPointer<Structure>>,
    pub(crate) object_prototype: Option<GcPointer<JsObject>>,
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
    pub(crate) regexp_object: Option<GcPointer<JsObject>>,
    pub(crate) array_buffer_prototype: Option<GcPointer<JsObject>>,
    pub(crate) array_buffer_structure: Option<GcPointer<Structure>>,
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
    use crate::options::Options;
    use crate::vm::symbol_table::Internable;
    use crate::vm::value::JsValue;
    use crate::vm::{arguments, Runtime};
    use crate::Platform;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_simple_async() {
        // start a runtime
        Platform::initialize();

        type JobType = dyn FnOnce(&mut Runtime);
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

        let mut global = starlight_runtime.global_object();

        // create a symbol for the functions name
        let name_symbol = "setImmediate".intern();

        // create a setImmediate Function
        // the setImmediate function is not part of the official ECMAspec but is implemented in older IE versions
        // it should work about the same as setTimeout(func, 0)
        // see also https://developer.mozilla.org/en-US/docs/Web/API/Window/setImmediate
        // it should serve pretty good as a simple as it gets first example of doing things async
        let arg_count = 0;
        let func = crate::vm::function::JsNativeFunction::new(
            &mut starlight_runtime,
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
            .put(
                &mut starlight_runtime,
                name_symbol,
                JsValue::new(func),
                true,
            )
            .ok()
            .expect("could not add func to global");

        // run the function
        let _outcome =
            match starlight_runtime.eval("setImmediate(() => {print('later')}); print('first');") {
                Ok(e) => e,
                Err(err) => panic!(
                    "func failed: {}",
                    err.to_string(&mut starlight_runtime)
                        .ok()
                        .expect("conversion failed")
                ),
            };

        if let Some(job) = todo.take() {
            job(&mut starlight_runtime);
        } else {
            panic!("did not get job")
        }
    }
}
