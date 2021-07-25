use crate::{define_op_builtins, gc::cell::GcCell, jsrt::VM_NATIVE_REFERENCES, vm::Lrc};
use std::{collections::HashMap, ptr::null};
use swc_common::{errors::Handler, input::StringInput, FileName, SourceMap};
use swc_ecmascript::parser::{Parser, Syntax};

use crate::{
    bytecompiler::{ByteCompiler, CompileError},
    gc::{
        cell::{GcPointer, Trace, Tracer},
        shadowstack::ShadowStack,
        Heap,
    },
    jsrt,
    vm::{
        arguments::{Arguments, JsArguments},
        environment::Environment,
        error::JsSyntaxError,
        function::JsVMFunction,
        init_es_config, BufferedError,
    },
};

use super::{
    array::JsArray,
    array_buffer::JsArrayBuffer,
    builder::{Builtin, ClassBuilder, ClassConstructor},
    class::JsClass,
    data_view::JsDataView,
    error::JsError,
    error::{JsRangeError, JsReferenceError, JsTypeError},
    function::JsNativeFunction,
    function::{JsFunction, JsGeneratorFunction},
    global::JsGlobal,
    interpreter::{frame::CallFrame, stack::Stack},
    number::JsNumber,
    object::{JsObject, ObjectTag},
    promise::JsPromise,
    string::JsString,
    string::JsStringObject,
    structure::Structure,
    symbol_table::JsSymbolObject,
    symbol_table::{self, Internable, JsSymbol, Symbol},
    value::JsValue,
    GlobalData, ModuleKind, MyEmiter, VirtualMachine, VirtualMachineRef,
};

use crate::jsrt::boolean::JsBoolean;
use crate::jsrt::date::Date;
use crate::jsrt::math::Math;
use crate::jsrt::regexp::RegExp;
use crate::jsrt::weak_ref::JsWeakRef;
use crate::jsrt::SelfHost;

use crate::gc::snapshot::deserializer::Deserializable;

// evalute context
pub struct Context {
    pub(crate) global_data: GlobalData,
    pub(crate) global_object: Option<GcPointer<JsObject>>,
    pub(crate) stack: Stack,
    pub(crate) vm: VirtualMachineRef,
    pub(crate) stacktrace: String,
    pub(crate) module_loader: Option<GcPointer<JsObject>>,
    pub(crate) modules: HashMap<String, ModuleKind>,
    pub(crate) symbol_table: HashMap<Symbol, GcPointer<JsSymbol>>,
}
impl Context {
    pub fn global_object(&mut self) -> GcPointer<JsObject> {
        self.global_object.unwrap()
    }

    pub fn modules(&mut self) -> &mut HashMap<String, ModuleKind> {
        &mut self.modules
    }

    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
    pub fn vm(&self) -> VirtualMachineRef {
        self.vm
    }
    // proxy heap
    pub fn heap(&mut self) -> &mut Heap {
        self.vm.heap()
    }

    // proxy shadowstack
    pub fn shadowstack<'a>(&self) -> &'a ShadowStack {
        self.vm.shadowstack()
    }

    pub fn module_loader(&mut self) -> Option<GcPointer<JsObject>> {
        self.module_loader
    }

    pub fn new_raw() -> Context {
        Self {
            global_data: GlobalData::default(),
            global_object: None,
            vm: VirtualMachineRef(null::<*mut VirtualMachine>() as *mut VirtualMachine),
            stack: Stack::new(),
            stacktrace: String::new(),
            module_loader: None,
            modules: HashMap::new(),
            symbol_table: HashMap::new(),
        }
    }

    pub fn new_empty(vm: &mut VirtualMachine) -> GcPointer<Context> {
        let mut context = Self {
            global_data: GlobalData::default(),
            global_object: None,
            vm: VirtualMachineRef(vm),
            stack: Stack::new(),
            stacktrace: String::new(),
            module_loader: None,
            modules: HashMap::new(),
            symbol_table: HashMap::new(),
        };
        let ctx = vm.heap().allocate(context);
        ctx
    }

    pub fn new(vm: &mut VirtualMachine) -> GcPointer<Context> {
        vm.gc.defer();
        let mut ctx = Context::new_empty(vm);
        ctx.global_object = Some(JsGlobal::new(ctx));
        ctx.init().expect("Context init failed");
        vm.contexts.push(ctx);
        vm.gc.undefer();
        vm.gc.collect_if_necessary();
        ctx
    }
}
impl GcPointer<Context> {
    pub fn register_native_reference(reference: usize) {
        unsafe {
            VM_NATIVE_REFERENCES.push(reference);
        }
    }

    pub fn remove_reference(reference: usize) {
        unsafe {
            let index = VM_NATIVE_REFERENCES
                .iter()
                .position(|r| *r == reference)
                .expect("Reference not found");
            VM_NATIVE_REFERENCES.remove(index);
        }
    }

    pub fn register_class<T>(mut self) -> Result<(), JsValue>
    where
        T: ClassConstructor + JsClass,
    {
        let name = T::class().name;
        let obj_proto = self.global_data().get_object_prototype();
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, &structure, T::class(), ObjectTag::Ordinary);

        let structure = Structure::new_indexed(self, Some(proto), false);
        let mut constructor = JsNativeFunction::new(self, name.intern(), T::raw_constructor, 1);

        def_native_property!(self, constructor, prototype, proto)?;
        def_native_property!(self, proto, constructor, constructor)?;

        let mut builder = ClassBuilder {
            constructor,
            prototype: proto,
            structure,
            context: self,
        };
        T::init(&mut builder)?;
        self.register_structure(name.intern(), structure);

        let mut global_object = self.global_object();
        def_native_property!(self, global_object, name.intern(), constructor)?;

        unsafe {
            VM_NATIVE_REFERENCES.push(T::class() as *const _ as _);
            VM_NATIVE_REFERENCES.push(T::raw_constructor as _);
        }
        Ok(())
    }

    pub fn register_builtin<T>(mut self) -> Result<(), JsValue>
    where
        T: Builtin,
    {
        T::init(self)?;
        Ok(())
    }

    pub fn register_structure(&mut self, name: Symbol, structure: GcPointer<Structure>) {
        self.global_data.register_structure(name, structure);
    }

    pub fn get_structure(&mut self, name: Symbol) -> Option<GcPointer<Structure>> {
        self.global_data.get_structure(name)
    }

    pub fn init(&mut self) -> Result<(), JsValue> {
        macro_rules! define_register_builtin {
            ($class: ident) => {
                self.register_builtin::<$class>()?;
            };
        }
        define_op_builtins!(define_register_builtin);
        self.init_module_loader();
        self.init_internal_modules();
        self.init_dollar();
        Ok(())
    }
}

impl GcPointer<Context> {
    /// Construct new type error from provided string.
    pub fn new_type_error(mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsTypeError::new(self, msg, None)
    }
    /// Construct new reference error from provided string.
    pub fn new_reference_error(mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsReferenceError::new(self, msg, None)
    }
    /// Construct new syntax error from provided string.
    pub fn new_syntax_error(mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsSyntaxError::new(self, msg, None)
    }
    /// Construct new range error from provided string.
    pub fn new_range_error(mut self, msg: impl AsRef<str>) -> GcPointer<JsObject> {
        let msg = JsString::new(self, msg);
        JsRangeError::new(self, msg, None)
    }
}

impl GcPointer<Context> {
    pub fn compile_function(
        mut self,
        name: &str,
        code: &str,
        params: &[String],
    ) -> Result<JsValue, CompileError> {
        let mut code = ByteCompiler::compile_code(self, params, "", code.to_owned(), false)?;
        code.get_jsobject().as_function_mut().as_vm_mut().code.name = name.intern();

        Ok(code)
    }
    /// Compile provided script into JS function. If error when compiling happens `SyntaxError` instance
    /// is returned.
    pub fn compile(
        mut self,
        path: &str,
        name: &str,
        script: &str,
        builtins: bool,
    ) -> Result<JsValue, CompileError> {
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
            Err(_e) => {
                // let msg = JsString::new(self, e.kind().msg());
                return Err(CompileError::NotYetImpl("parser error".to_string()));
            }
        };
        let mut code = ByteCompiler::compile_script(
            self,
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
        mut self,
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

        let mut code = ByteCompiler::compile_module(
            self,
            path,
            &std::path::Path::new(&path)
                .canonicalize()
                .unwrap()
                .parent()
                .map(|x| x.to_str().unwrap().to_string())
                .unwrap_or_else(|| "".to_string()),
            name,
            &module,
        )
        .map_err(|e| self.new_syntax_error(format!("Compile Error {:?}", e)))?;
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
        mut self,
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
            let mut code = ByteCompiler::compile_eval(
                self,
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
            )
            .map_err(|e| self.new_syntax_error(format!("Compile Error {:?}", &e)))?;
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
        mut self,
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
            let mut code = ByteCompiler::compile_module(
                self,
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
            )
            .map_err(|e| self.new_syntax_error(format!("Compile Error {:?}", &e)))?;
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

    pub fn init_module_loader(mut self) {
        let loader = JsNativeFunction::new(self, "@loader".intern(), jsrt::module_load, 1);
        self.module_loader = Some(loader);
    }

    pub fn init_internal_modules(&mut self) {
        self.add_module(
            "std",
            ModuleKind::NativeUninit(crate::jsrt::jsstd::init_js_std),
        )
        .unwrap();
        assert!(self.modules.contains_key("std"));
    }

    pub fn add_module(
        mut self,
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

    /// Return [Symbol](crate::vm::symbol_table::Symbol) description.
    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(key) | Symbol::Private(key) => {
                symbol_table::symbol_table().description(key).to_owned()
            }
            Symbol::Index(x) => x.to_string(),
        }
    }

    pub(crate) fn schedule_async<F>(mut self, job: F) -> Result<(), JsValue>
    where
        F: FnOnce(GcPointer<Context>) + 'static,
    {
        if let Some(scheduler) = &self.vm.sched_async_func {
            scheduler(Box::new(job));
            Ok(())
        } else {
            Err(JsValue::encode_object_value(JsString::new(self, "In order to use async you have to init the VirtualMachineOptions with with_async_scheduler()")))
        }
    }

    /// Get stacktrace. If there was no error then returned string is empty.
    pub fn take_stacktrace(&mut self) -> String {
        std::mem::take(&mut self.stacktrace)
    }
}

impl GcCell for Context {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

unsafe impl Trace for Context {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.global_object().trace(visitor);
        self.global_data.trace(visitor);
        self.stack.trace(visitor);
        self.module_loader.trace(visitor);
        self.modules.trace(visitor);
        // self.symbol_table.trace(visitor);
    }
}
