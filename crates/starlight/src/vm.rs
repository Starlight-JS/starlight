use std::ops::{Deref, DerefMut};

use super::codegen::*;
use crate::{
    heap::{cell::GcPointer, cell::Trace, Heap, SimpleMarkingConstraint, SlotVisitor},
    jsrt::object::{object_constructor, object_to_string},
};
use arguments::Arguments;
use error::JsSyntaxError;
use function::JsVMFunction;
use std::{fmt::Display, io::Write, sync::RwLock};
use string::JsString;
use swc_common::{
    errors::{DiagnosticBuilder, Emitter, Handler},
    sync::Lrc,
};
use swc_common::{FileName, SourceMap};
use swc_ecmascript::parser::*;
#[macro_use]
pub mod class;
#[macro_use]
pub mod method_table;
pub mod arguments;
pub mod array;
pub mod array_storage;
pub mod attributes;
pub mod bigint;
pub mod code_block;
pub mod error;
pub mod function;
pub mod global;
pub mod indexed_elements;
pub mod interpreter;
pub mod object;
pub mod property_descriptor;
pub mod slot;
pub mod string;
pub mod structure;
pub mod symbol_table;
pub mod thread;
pub mod tracingjit;
pub mod value;
use attributes::*;
use object::*;
use property_descriptor::*;
use value::*;

pub struct GcParams {
    pub(crate) nmarkers: u32,
    pub(crate) track_allocations: bool,
    pub(crate) parallel_marking: bool,
}

pub struct RuntimeParams {
    pub(crate) dump_bytecode: bool,
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
            track_allocations: false,
            parallel_marking: true,
            nmarkers: 4,
        }
    }
}

impl GcParams {
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

pub struct Runtime {
    pub(crate) heap: Box<Heap>,
    pub(crate) stack: Stack,
    pub(crate) global_data: GlobalData,
    pub(crate) global_object: Option<GcPointer<JsObject>>,
    pub(crate) external_references: Option<&'static [usize]>,
    pub(crate) options: RuntimeParams,
}

impl Runtime {
    pub fn compile(&mut self, name: &str, script: &str) -> Result<JsValue, JsValue> {
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
        let code = Compiler::compile_script(&mut *vmref, &script);

        //code.display_to(&mut OutBuf).unwrap();

        let envs = Structure::new_indexed(self, Some(self.global_object()), false);
        let env = JsObject::new(self, envs, JsObject::get_class(), ObjectTag::Ordinary);
        let fun = JsVMFunction::new(self, code, env);
        return Ok(JsValue::encode_object_value(fun));
    }
    pub fn eval(&mut self, force_strict: bool, script: &str) -> Result<JsValue, JsValue> {
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
            let mut code = Compiler::compile_script(&mut *vmref, &script);
            code.strict = code.strict || force_strict;
            //code.display_to(&mut OutBuf).unwrap();

            let envs = Structure::new_indexed(self, Some(self.global_object()), false);
            let env = JsObject::new(self, envs, JsObject::get_class(), ObjectTag::Ordinary);
            let mut fun = JsVMFunction::new(self, code, env);

            let mut args = Arguments::new(self, JsValue::encode_undefined_value(), 0);
            keep_on_stack!(&fun, &args);
            fun.as_function_mut().call(self, &mut args)
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
    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(key) => symbol_table::symbol_table().description(key).to_owned(),
            Symbol::Index(x) => x.to_string(),
        }
    }
    pub fn heap(&mut self) -> &mut Heap {
        &mut self.heap
    }
    pub(crate) fn new_empty(
        gc_params: GcParams,
        options: RuntimeParams,
        external_references: Option<&'static [usize]>,
    ) -> Box<Self> {
        let heap = Box::new(Heap::new(gc_params));
        let mut this = Box::new(Self {
            heap,
            options,
            stack: Stack::new(),
            global_object: None,
            global_data: GlobalData::default(),
            external_references,
        });
        let vm = &mut *this as *mut Runtime;
        this.heap.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
                rt.stack.trace(visitor);
            },
        ));

        this
    }
    pub fn new(
        options: RuntimeParams,
        gc_params: GcParams,
        external_references: Option<&'static [usize]>,
    ) -> Box<Runtime> {
        let heap = Box::new(Heap::new(gc_params));
        let mut this = Box::new(Self {
            heap,
            options,
            stack: Stack::new(),
            global_object: None,
            global_data: GlobalData::default(),
            external_references,
        });
        let vm = &mut *this as *mut Runtime;
        this.heap.defer();
        this.heap.add_constraint(SimpleMarkingConstraint::new(
            "Mark VM roots",
            move |visitor| {
                let rt = unsafe { &mut *vm };
                rt.global_object.trace(visitor);
                rt.global_data.trace(visitor);
                rt.stack.trace(visitor);
            },
        ));
        this.global_data.empty_object_struct = Some(Structure::new_indexed(&mut this, None, false));
        let s = Structure::new_unique_indexed(&mut this, None, false);
        let mut proto = JsObject::new(&mut this, s, JsObject::get_class(), ObjectTag::Ordinary);
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

        let name = "Object".intern();
        let mut obj_constructor = JsNativeFunction::new(&mut this, name, object_constructor, 1);
        let _ = obj_constructor.define_own_property(
            &mut this,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::from(proto.clone()), NONE),
            false,
        );
        let _ = proto.define_own_property(
            &mut this,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::from(obj_constructor.clone()), W | C),
            false,
        );
        let obj_to_string =
            JsNativeFunction::new(&mut this, "toString".intern(), object_to_string, 0);
        let _ = proto.define_own_property(
            &mut this,
            "toString".intern(),
            &*DataDescriptor::new(JsValue::from(obj_to_string), W | C),
            false,
        );
        let name = "Object".intern();
        this.global_data
            .empty_object_struct
            .as_mut()
            .unwrap()
            .change_prototype_with_no_transition(proto.clone());
        this.global_data.number_structure = Some(Structure::new_indexed(&mut this, None, false));
        keep_on_stack!(&mut proto);
        this.init_error(proto.clone());
        keep_on_stack!(&mut proto);
        let _ = this.global_object().define_own_property(
            &mut this,
            name,
            &*DataDescriptor::new(JsValue::from(obj_constructor), W | C),
            false,
        );
        keep_on_stack!(&mut proto);
        this.init_array(proto.clone());
        keep_on_stack!(&mut proto);
        this.init_func(proto);
        this.init_builtin();
        this.heap.undefer();

        this
    }

    pub fn global_object(&self) -> GcPointer<JsObject> {
        unwrap_unchecked(self.global_object.clone())
    }

    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
}

use starlight_derive::GcTrace;
use wtf_rs::{keep_on_stack, unwrap_unchecked};

use self::{
    function::JsNativeFunction,
    global::JsGlobal,
    interpreter::stack::Stack,
    object::JsObject,
    structure::Structure,
    symbol_table::{Internable, Symbol},
};

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
struct MyEmiter(BufferedError);
impl Emitter for MyEmiter {
    fn emit(&mut self, db: &DiagnosticBuilder<'_>) {
        let z = &(self.0).0;
        for msg in &db.message {
            z.write().unwrap().push_str(&msg.0);
        }
    }
}
struct OutBuf;

impl std::fmt::Write for OutBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        print!("{}", s);
        Ok(())
    }
}
