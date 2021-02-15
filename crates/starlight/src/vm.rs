use super::runtime::attributes::*;
use std::{fmt::Display, io::Write, sync::RwLock};
use std::{ops::DerefMut, ptr::null_mut};
use swc_common::{
    errors::{DiagnosticBuilder, Emitter, Handler},
    sync::Lrc,
};
use swc_common::{FileName, SourceMap};
use swc_ecmascript::parser::*;
use wtf_rs::{object_offsetof, unwrap_unchecked};

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
use crate::{
    frontend::Compiler,
    gc::space::Space,
    heap::{
        cell::{Cell, Gc, Trace, Tracer},
        constraint::SimpleMarkingConstraint,
        Allocator,
    },
    interpreter::frame::FrameBase,
    jsrt::error::{
        error_constructor, error_to_string, eval_error_constructor, reference_error_constructor,
        type_error_constructor,
    },
    runtime::{
        arguments::Arguments,
        error::{JsError, JsEvalError, JsReferenceError, JsTypeError},
        function::{JsNativeFunction, JsVMFunction},
        global::JsGlobal,
        object::{JsObject, ObjectTag},
        property_descriptor::DataDescriptor,
        string::JsString,
        structure::Structure,
        symbol::Symbol,
        value::JsValue,
    },
    symbol_table::SymbolTable,
};
use lexer::Lexer;
use structopt::*;

#[derive(StructOpt)]
pub struct Options {
    #[structopt(long, help = "Write barrier inline buffer size.", default_value = "32")]
    pub write_barrier_buffer_size: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            write_barrier_buffer_size: 32,
        }
    }
}

#[repr(C)]
pub struct VirtualMachine {
    return_value: JsValue,
    thrown_error: JsValue,
    global_object: Option<Gc<JsObject>>,
    acc: JsValue,
    pub(crate) stack_start: *mut JsValue,
    pub(crate) stack_end: *mut JsValue,
    pub(crate) stack: *mut JsValue,
    space: Box<Space>,
    interner: SymbolTable,
    global_data: Box<GlobalData>,
    pub(crate) frame: *mut FrameBase,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VirtualMachineRef(*mut VirtualMachine);

impl VirtualMachineRef {
    pub fn dispose(this: Self) {
        unsafe {
            let _ = Box::from_raw(this.0);
        }
    }
}

impl DerefMut for VirtualMachineRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}
impl std::ops::Deref for VirtualMachineRef {
    type Target = VirtualMachine;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

pub trait IntoSymbol {
    fn into_symbol(self, vm: &mut VirtualMachine) -> Symbol;
}

impl IntoSymbol for u32 {
    fn into_symbol(self, _vm: &mut VirtualMachine) -> Symbol {
        Symbol::Indexed(self)
    }
}
impl IntoSymbol for &str {
    fn into_symbol(self, vm: &mut VirtualMachine) -> Symbol {
        vm.interner.lookup(self)
    }
}
impl IntoSymbol for String {
    fn into_symbol(self, vm: &mut VirtualMachine) -> Symbol {
        vm.interner.lookup(self)
    }
}

struct OutBuf;

impl std::fmt::Write for OutBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        print!("{}", s);
        Ok(())
    }
}

impl VirtualMachine {
    pub fn eval(&mut self, script: &str) -> Result<JsValue, JsValue> {
        let res = {
            let ctx = self.space().new_local_context();
            let cm: Lrc<SourceMap> = Default::default();
            let e = BufferedError::default();

            let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));
            // Real usage
            // let fm = cm
            //     .load_file(Path::new("test.js"))
            //     .expect("failed to load test.js");
            let fm = cm.new_source_file(FileName::Custom("<script>".into()), script.into());
            let lexer = Lexer::new(
                // We want to parse ecmascript
                Syntax::Es(Default::default()),
                // JscTarget defaults to es5
                Default::default(),
                StringInput::from(&*fm),
                None,
            );

            let mut parser = Parser::new_from(lexer);

            for e in parser.take_errors() {
                e.into_diagnostic(&handler).emit();
            }

            let script = match parser.parse_script() {
                Ok(script) => script,
                Err(e) => {
                    todo!("throw error");
                }
            };

            let code = ctx.new_local(Compiler::compile_script(VirtualMachineRef(self), &script));

            code.display_to(&mut OutBuf).unwrap();

            let envs = Structure::new_indexed(self, Some(self.global_object()), false);
            let env = JsObject::new(self, envs, JsObject::get_class(), ObjectTag::Ordinary);
            let mut fun = ctx.new_local(JsVMFunction::new(self, *code, env));
            let mut args = ctx.new_local(Arguments::new(self, JsValue::undefined(), 0));
            fun.as_function_mut().call(self, &mut args)
        };

        res
    }
    pub fn description(&self, sym: Symbol) -> String {
        match sym {
            Symbol::Key(x) => unsafe { (*x).to_string() },
            Symbol::Indexed(x) => x.to_string(),
        }
    }

    pub(crate) fn try_cache(
        &mut self,
        s: Gc<Structure>,
        obj: Gc<JsObject>,
    ) -> Option<Gc<JsObject>> {
        if Gc::ptr_eq(unwrap_unchecked(obj.get_structure()), s) {
            return Some(obj);
        }

        let mut current = obj.prototype();
        while let Some(cur) = current {
            if Gc::ptr_eq(unwrap_unchecked(cur.get_structure()), s) {
                return Some(cur);
            }
            current = cur.prototype();
        }
        None
    }
    pub fn intern(&mut self, val: impl IntoSymbol) -> Symbol {
        val.into_symbol(self)
    }

    pub fn push(&mut self, val: JsValue) {
        unsafe {
            if self.stack == self.stack_end {
                panic!("Stack overflow");
            }

            self.stack.write(val);
            self.stack = self.stack.add(1);
        }
    }
    pub fn upop(&mut self) -> JsValue {
        unsafe {
            self.stack = self.stack.sub(1);
            self.stack.read()
        }
    }
    pub fn upush(&mut self, val: JsValue) {
        unsafe {
            self.stack.write(val);
            self.stack = self.stack.add(1);
        }
    }
    pub fn pop(&mut self) -> Option<JsValue> {
        if self.stack == self.stack_start {
            return None;
        }
        unsafe {
            self.stack = self.stack.sub(1);
            Some(self.stack.read())
        }
    }

    pub fn global_data(&self) -> &GlobalData {
        &self.global_data
    }
    pub fn new(opts: Options) -> VirtualMachineRef {
        let space = Space::new();
        let stack = Vec::<JsValue>::with_capacity(16 * 1024);
        let ptr = stack.as_ptr() as *mut JsValue;
        std::mem::forget(stack);
        let stack = ptr;
        let stack_end = unsafe { ptr.add(16 * 1024) };

        let mut this = VirtualMachineRef(Box::into_raw(Box::new(Self {
            space,
            interner: SymbolTable::new(),
            global_data: Box::new(GlobalData::default()),
            global_object: None,
            thrown_error: JsValue::undefined(),
            return_value: JsValue::undefined(),
            stack_start: stack,
            frame: null_mut(),
            stack,
            stack_end,
            acc: JsValue::undefined(),
        })));
        let c = this;
        this.space.add_constraint(SimpleMarkingConstraint::new(
            "VM marking",
            move |tracer| unsafe {
                let vm = c;
                (*vm).global_data.trace(tracer);
                (*vm).global_object.trace(tracer);
                (*vm).thrown_error.trace(tracer);
                (*vm).return_value.trace(tracer);
                unsafe {
                    let mut current = (*vm).frame as *const FrameBase;
                    while !current.is_null() {
                        (*current).trace(tracer);
                        current = (*current).prev;
                    }
                }
            },
        ));

        this.global_data.empty_object_struct = Some(Structure::new_indexed(&mut this, None, false));
        let s = this.global_data().empty_object_struct.unwrap();
        let proto = JsObject::new(&mut this, s, JsObject::get_class(), ObjectTag::Ordinary);
        this.global_data.object_prototype = Some(proto);
        this.global_data.function_struct = Some(Structure::new_indexed(&mut this, None, false));
        this.global_data.normal_arguments_structure =
            Some(Structure::new_indexed(&mut this, None, false));
        this.global_object = Some(JsGlobal::new(&mut this));
        this.init_error(proto);

        this
    }
    pub fn global_object(&self) -> Gc<JsObject> {
        unwrap_unchecked(self.global_object)
    }
    pub fn space(&mut self) -> &mut Space {
        &mut self.space
    }

    pub fn space_offset() -> usize {
        object_offsetof!(Self, space)
    }
    pub fn get_this(&self) -> JsValue {
        let mut ret = JsValue::new(self.global_object.unwrap());
        if !self.frame.is_null() {
            ret = unsafe { (*self.frame).this_obj };
        }
        ret
    }
    pub fn get_scope(&self) -> Gc<JsObject> {
        let mut cf = null_mut();
        let mut cur = self.frame;
        while !cur.is_null() {
            unsafe {
                if (*cur).is_bcode != 0 {
                    cf = cur;
                    break;
                }
            }
            cur = unsafe { (*cur).prev };
        }
        if !cf.is_null() {
            unsafe { (*cf).scope.as_cell().downcast().expect("Scope expected") }
        } else {
            self.global_object.unwrap()
        }
    }
    fn init_error(&mut self, obj_proto: Gc<JsObject>) {
        self.global_data.eval_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.range_error_structure = Some(Structure::new_indexed(self, None, false));
        self.global_data.reference_error_structure =
            Some(Structure::new_indexed(self, None, false));
        self.global_data.type_error_structure = Some(Structure::new_indexed(self, None, false));
        let structure = Structure::new_unique_with_proto(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, structure, JsError::get_class(), ObjectTag::Ordinary);
        let e = self.intern("Error");
        let mut ctor = JsNativeFunction::new(self, e, error_constructor, 1);
        let _ = ctor.define_own_property(
            self,
            Symbol::prototype(),
            &*DataDescriptor::new(JsValue::new(proto), NONE),
            false,
        );
        proto.set_class_value(JsError::get_class());
        let _ = proto.define_own_property(
            self,
            Symbol::constructor(),
            &*DataDescriptor::new(JsValue::new(ctor), W | C),
            false,
        );

        let n = self.intern("name");
        let s = JsString::new(self, "Error");
        let e = JsString::new(self, "");
        let m = self.intern("message");
        let _ = proto.define_own_property(
            self,
            n,
            &*DataDescriptor::new(JsValue::new(s), W | C),
            false,
        );

        let _ = proto.define_own_property(
            self,
            m,
            &*DataDescriptor::new(JsValue::new(e), W | C),
            false,
        );
        let to_str = JsNativeFunction::new(self, Symbol::toString(), error_to_string, 0);
        let _ = proto.define_own_property(
            self,
            Symbol::toString(),
            &*DataDescriptor::new(JsValue::new(to_str), W | C),
            false,
        );
        let sym = self.intern("Error");
        let _ = self.global_object().define_own_property(
            self,
            sym,
            &*DataDescriptor::new(JsValue::new(proto), W | C),
            false,
        );
        self.global_data.error = Some(proto);
        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                structure,
                JsEvalError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .eval_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = self.intern("EvalError");
            let mut sub_ctor = JsNativeFunction::new(self, sym, eval_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                Symbol::prototype(),
                &*DataDescriptor::new(JsValue::new(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                Symbol::constructor(),
                &*DataDescriptor::new(JsValue::new(sub_ctor), W | C),
                false,
            );

            let n = self.intern("name");
            let s = JsString::new(self, "EvalError");
            let e = JsString::new(self, "");
            let m = self.intern("message");
            let _ = sub_proto.define_own_property(
                self,
                n,
                &*DataDescriptor::new(JsValue::new(s), W | C),
                false,
            );

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::new(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, Symbol::toString(), error_to_string, 0);
            let _ = sub_proto.define_own_property(
                self,
                Symbol::toString(),
                &*DataDescriptor::new(JsValue::new(to_str), W | C),
                false,
            );
            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::new(sub_proto), W | C),
                false,
            );

            self.global_data.eval_error = Some(sub_proto);
        }

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                structure,
                JsTypeError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .type_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = self.intern("TypeError");
            let mut sub_ctor = JsNativeFunction::new(self, sym, type_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                Symbol::prototype(),
                &*DataDescriptor::new(JsValue::new(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                Symbol::constructor(),
                &*DataDescriptor::new(JsValue::new(sub_ctor), W | C),
                false,
            );

            let n = self.intern("name");
            let s = JsString::new(self, "TypeError");
            let e = JsString::new(self, "");
            let m = self.intern("message");
            let _ = sub_proto
                .define_own_property(
                    self,
                    n,
                    &*DataDescriptor::new(JsValue::new(s), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::new(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, Symbol::toString(), error_to_string, 0);
            let _ = sub_proto
                .define_own_property(
                    self,
                    Symbol::toString(),
                    &*DataDescriptor::new(JsValue::new(to_str), W | C),
                    false,
                )
                .unwrap_or_else(|_| panic!());
            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::new(sub_proto), W | C),
                false,
            );

            self.global_data.type_error = Some(sub_proto);
        }

        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                structure,
                JsReferenceError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .reference_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = self.intern("ReferenceError");
            let mut sub_ctor = JsNativeFunction::new(self, sym, reference_error_constructor, 1);
            let _ = sub_ctor.define_own_property(
                self,
                Symbol::prototype(),
                &*DataDescriptor::new(JsValue::new(sub_proto), NONE),
                false,
            );
            let _ = sub_proto.define_own_property(
                self,
                Symbol::constructor(),
                &*DataDescriptor::new(JsValue::new(sub_ctor), W | C),
                false,
            );

            let n = self.intern("name");
            let s = JsString::new(self, "ReferenceError");
            let e = JsString::new(self, "");
            let m = self.intern("message");
            let _ = sub_proto.define_own_property(
                self,
                n,
                &*DataDescriptor::new(JsValue::new(s), W | C),
                false,
            );

            let _ = sub_proto.define_own_property(
                self,
                m,
                &*DataDescriptor::new(JsValue::new(e), W | C),
                false,
            );
            let to_str = JsNativeFunction::new(self, Symbol::toString(), error_to_string, 0);
            let _ = sub_proto.define_own_property(
                self,
                Symbol::toString(),
                &*DataDescriptor::new(JsValue::new(to_str), W | C),
                false,
            );

            let _ = self.global_object().define_own_property(
                self,
                sym,
                &*DataDescriptor::new(JsValue::new(sub_proto), W | C),
                false,
            );

            self.global_data.reference_error = Some(sub_proto);
        }
    }
}

impl<T: Cell> Allocator<T> for VirtualMachine {
    type Result = Gc<T>;
    fn allocate(&mut self, value: T) -> Self::Result {
        self.space().alloc(value)
    }
}

use starlight_derive::Trace;

#[derive(Default, Trace)]
pub struct GlobalData {
    pub(crate) normal_arguments_structure: Option<Gc<Structure>>,
    pub(crate) empty_object_struct: Option<Gc<Structure>>,
    pub(crate) function_struct: Option<Gc<Structure>>,
    pub(crate) object_prototype: Option<Gc<JsObject>>,
    pub(crate) number_prototype: Option<Gc<JsObject>>,
    pub(crate) string_prototype: Option<Gc<JsObject>>,
    pub(crate) boolean_prototype: Option<Gc<JsObject>>,
    pub(crate) symbol_prototype: Option<Gc<JsObject>>,
    pub(crate) error: Option<Gc<JsObject>>,
    pub(crate) type_error: Option<Gc<JsObject>>,
    pub(crate) reference_error: Option<Gc<JsObject>>,
    pub(crate) range_error: Option<Gc<JsObject>>,
    pub(crate) syntax_error: Option<Gc<JsObject>>,
    pub(crate) internal_error: Option<Gc<JsObject>>,
    pub(crate) eval_error: Option<Gc<JsObject>>,

    pub(crate) error_structure: Option<Gc<Structure>>,
    pub(crate) range_error_structure: Option<Gc<Structure>>,
    pub(crate) reference_error_structure: Option<Gc<Structure>>,
    pub(crate) syntax_error_structure: Option<Gc<Structure>>,
    pub(crate) type_error_structure: Option<Gc<Structure>>,
    pub(crate) uri_error_structure: Option<Gc<Structure>>,
    pub(crate) eval_error_structure: Option<Gc<Structure>>,
}

impl GlobalData {
    pub fn get_function_struct(&self) -> Gc<Structure> {
        self.function_struct.unwrap()
    }

    pub fn get_object_prototype(&self) -> Gc<JsObject> {
        self.object_prototype.unwrap()
    }
}

impl Drop for VirtualMachine {
    fn drop(&mut self) {
        unsafe {
            let _ = Vec::from_raw_parts(self.stack_start, 0, 16 * 1024);
        }
    }
}
