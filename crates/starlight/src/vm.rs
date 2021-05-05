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
pub mod tracingjit;
pub mod value;
use crate::gc::snapshot::{deserializer::*, serializer::*};
use attributes::*;
use object::*;
use property_descriptor::*;
use value::*;

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
        match self {
            Self::Initialized(x) => x.trace(visitor),
            _ => (),
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

pub struct GcParams {
    pub(crate) nmarkers: u32,
    #[allow(dead_code)]
    pub(crate) heap_size: usize,
    pub(crate) conservative_marking: bool,
    #[allow(dead_code)]
    pub(crate) track_allocations: bool,
    pub(crate) parallel_marking: bool,
}

pub struct RuntimeParams {
    pub(crate) dump_bytecode: bool,
    #[allow(dead_code)]
    pub(crate) inline_caches: bool,
}
impl Default for RuntimeParams {
    fn default() -> Self {
        Self {
            dump_bytecode: false,
            inline_caches: true,
        }
    }
}
impl RuntimeParams {
    pub fn with_inline_caching(mut self, enabled: bool) -> Self {
        self.inline_caches = enabled;
        self
    }
    pub fn with_dump_bytecode(mut self, enabled: bool) -> Self {
        self.dump_bytecode = enabled;
        self
    }
}

impl Default for GcParams {
    fn default() -> Self {
        Self {
            heap_size: 1 * 1024 * 1024 * 1024,
            conservative_marking: false,
            track_allocations: false,
            parallel_marking: true,
            nmarkers: 4,
        }
    }
}

impl GcParams {
    pub fn with_heap_size(mut self, mut size: usize) -> Self {
        if size < 256 * 1024 {
            size = 256 * 1024
        };
        self.heap_size = size;
        self
    }
    pub fn with_conservative_marking(mut self, enabled: bool) -> Self {
        self.conservative_marking = enabled;
        self
    }
    pub fn with_marker_threads(mut self, n: u32) -> Self {
        assert!(self.parallel_marking, "Enable parallel marking first");
        self.nmarkers = n;
        if n == 0 {
            panic!("Can't set zero marker threads");
        }
        self
    }

    pub fn with_parallel_marking(mut self, cond: bool) -> Self {
        self.parallel_marking = cond;
        self.nmarkers = 4;
        self
    }

    pub fn with_track_allocations(mut self, cond: bool) -> Self {
        self.track_allocations = cond;
        self
    }
}

/// JavaScript runtime instance.
pub struct Runtime {
    pub(crate) gc: Heap,
    pub(crate) stack: Stack,
    pub(crate) global_data: GlobalData,
    pub(crate) global_object: Option<GcPointer<JsObject>>,
    pub(crate) external_references: Option<&'static [usize]>,
    pub(crate) options: RuntimeParams,
    pub(crate) shadowstack: ShadowStack,
    pub(crate) stacktrace: String,
    pub(crate) symbol_table: HashMap<Symbol, GcPointer<JsSymbol>>,
    pub(crate) module_loader: Option<GcPointer<JsObject>>,
    pub(crate) modules: HashMap<String, ModuleKind>,
    #[cfg(feature = "perf")]
    pub(crate) perf: perf::Perf,
}

impl Runtime {
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
                    result.push_str(&format!(" at '<native code>\n"));
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

        let mut code =
            ByteCompiler::compile_script(&mut *vmref, &script, path.to_owned(), builtins);
        code.name = name.intern();
        //code.display_to(&mut OutBuf).unwrap();

        let env = Environment::new(self, 0);
        let fun = JsVMFunction::new(self, code, env);
        return Ok(JsValue::encode_object_value(fun));
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

        let mut parser = Parser::new(
            Syntax::Es(Default::default()),
            StringInput::from(&*fm),
            None,
        );

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

        let mut code = ByteCompiler::compile_module(&mut *vmref, path, name, &module);
        code.name = name.intern();

        let env = Environment::new(self, 0);
        let fun = JsVMFunction::new(self, code, env);
        return Ok(JsValue::encode_object_value(fun));
    }

    /// Tries to evaluate provided `script`. If error when parsing or execution occurs then `Err` with exception value is returned.
    ///
    ///
    ///
    /// TODO: Return script execution result. Right now just `undefined` value is returned.
    pub fn eval(
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
                path.map(|x| x.to_owned()).unwrap_or_else(|| String::new()),
                builtins,
            );
            code.strict = code.strict || force_strict;
            // code.file_name = path.map(|x| x.to_owned()).unwrap_or_else(|| String::new());
            //code.display_to(&mut OutBuf).unwrap();
            let stack = self.shadowstack();

            letroot!(env = stack, Environment::new(self, 0));
            letroot!(fun = stack, JsVMFunction::new(self, code, *env));
            letroot!(func = stack, *&*fun);
            letroot!(
                args = stack,
                Arguments::new(JsValue::encode_undefined_value(), &mut [])
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
        options: RuntimeParams,
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
        this.global_data.object_prototype = Some(proto.clone());
        this.global_data.function_struct = Some(Structure::new_indexed(&mut this, None, false));
        this.global_data.normal_arguments_structure =
            Some(Structure::new_indexed(&mut this, None, false));
        this.global_object = Some(JsGlobal::new(&mut this));
        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto.clone());

        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto.clone());
        this.global_data.number_structure = Some(Structure::new_indexed(&mut this, None, false));
        this.init_func(proto);
        this.init_error(proto.clone());
        this.init_array(proto.clone());
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
        this.init_self_hosted();
        let loader = JsNativeFunction::new(&mut this, "@loader".intern(), jsrt::module_load, 1);
        this.module_loader = Some(loader);
        this.add_module(
            "std",
            ModuleKind::NativeUninit(crate::jsrt::jsstd::init_js_std),
        )
        .unwrap();
        assert!(this.modules.contains_key("std"));
        this.gc.undefer();
        this.gc.collect_if_necessary();
        this
    }
    /// Get stacktrace. If there was no error then returned string is empty.
    pub fn take_stacktrace(&mut self) -> String {
        std::mem::replace(&mut self.stacktrace, String::new())
    }
    pub(crate) fn new_empty(
        gc: Heap,
        options: RuntimeParams,
        external_references: Option<&'static [usize]>,
    ) -> Box<Self> {
        let mut this = Box::new(Self {
            gc,
            options,
            modules: HashMap::new(),
            stack: Stack::new(),
            global_object: None,
            global_data: GlobalData::default(),
            external_references,
            stacktrace: String::new(),
            shadowstack: ShadowStack::new(),
            #[cfg(feature = "perf")]
            perf: perf::Perf::new(),
            symbol_table: HashMap::new(),
            module_loader: None,
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
            },
        ));

        this
    }
    /// Create new JS runtime with `MiGC` set as GC.
    pub fn new(
        options: RuntimeParams,
        gc_params: GcParams,
        external_references: Option<&'static [usize]>,
    ) -> Box<Runtime> {
        Self::with_heap(default_heap(gc_params), options, external_references)
    }

    /// Obtain global object reference.
    pub fn global_object(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.global_object.clone())
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
}

impl GlobalData {
    pub fn get_function_struct(&self) -> GcPointer<Structure> {
        unwrap_unchecked(self.function_struct.clone())
    }

    pub fn get_object_prototype(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.object_prototype.clone())
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
    let fm = cm.new_source_file(FileName::Custom("<script>".into()), script.into());

    let mut parser = Parser::new(
        Syntax::Es(Default::default()),
        StringInput::from(&*fm),
        None,
    );

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
