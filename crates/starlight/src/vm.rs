use super::runtime::attributes::*;
use crate::jsrt::{
    array::{array_from, array_join, array_of, array_pop, array_push},
    error::range_error_constructor,
    function::{function_bind, function_prototype, function_to_string},
    object::*,
};
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
    gc::{handle::Handle, heap::Heap},
    heap::{
        cell::{Cell, Gc, Trace, Tracer},
        constraint::SimpleMarkingConstraint,
        Allocator,
    },
    interpreter::frame::FrameBase,
    jsrt::{
        array::{array_ctor, array_is_array},
        error::{
            error_constructor, error_to_string, eval_error_constructor,
            reference_error_constructor, type_error_constructor,
        },
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

pub struct Options {
    pub dump_bytecode: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            dump_bytecode: false,
        }
    }
}

#[repr(C)]
pub struct VirtualMachine {
    pub(crate) options: Options,
    return_value: JsValue,
    thrown_error: JsValue,
    global_object: Option<Gc<JsObject>>,
    acc: JsValue,
    pub(crate) stack_start: *mut JsValue,
    pub(crate) stack_end: *mut JsValue,
    pub(crate) stack: *mut JsValue,
    space: Box<Heap>,
    interner: SymbolTable,
    stacktrace: Option<String>,
    global_data: Box<GlobalData>,
    stack_map: memmap2::MmapMut,
    pub(crate) frame: *mut FrameBase,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VirtualMachineRef(pub(crate) *mut VirtualMachine);

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
    pub fn take_stacktrace(&mut self) -> Option<String> {
        self.stacktrace.take()
    }

    pub fn set_stacktrace(&mut self, s: &str) {
        self.stacktrace = Some(s.to_string());
    }

    pub fn append_stacktrace(&mut self, s: &str) {
        if let Some(ref mut trace) = self.stacktrace {
            trace.push_str(&format!("\n{}", s));
        } else {
            self.set_stacktrace(s);
        }
    }
    pub fn compile(
        &mut self,
        force_strict: bool,
        code: &str,
        name: &str,
    ) -> Result<Gc<JsObject>, JsValue> {
        let cm: Lrc<SourceMap> = Default::default();
        let e = BufferedError::default();

        let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));
        // Real usage
        // let fm = cm
        //     .load_file(Path::new("test.js"))
        //     .expect("failed to load test.js");
        let fm = cm.new_source_file(FileName::Custom("<script>".into()), code.into());
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
                return Err(JsValue::new(JsString::new(self, "parse error")));
            }
        };
        self.space.defer_gc();
        let vmref = VirtualMachineRef(self);
        let mut code = Handle::new(self.space(), Compiler::compile_script(vmref, &script));
        code.strict = code.strict || force_strict;
        code.name = self.intern_or_known_symbol(name);

        let envs = Structure::new_indexed(self, Some(self.global_object()), false);
        let env = JsObject::new(self, envs, JsObject::get_class(), ObjectTag::Ordinary);
        let fun = JsVMFunction::new(self, *code, env);
        self.space.undefer_gc();
        return Ok(fun);
    }
    pub fn eval(&mut self, force_strict: bool, script: &str) -> Result<JsValue, JsValue> {
        let res = {
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
            let vmref = VirtualMachineRef(self);

            let mut code = Handle::new(self.space(), Compiler::compile_script(vmref, &script));
            code.strict = code.strict || force_strict;
            code.display_to(&mut OutBuf).unwrap();

            let envs = Structure::new_indexed(self, Some(self.global_object()), false);
            let env = JsObject::new(self, envs, JsObject::get_class(), ObjectTag::Ordinary);
            let fun = JsVMFunction::new(self, *code, env);
            let mut fun = Handle::new(self.space(), fun);
            let args = Arguments::new(self, JsValue::undefined(), 0);
            let mut args = Handle::new(self.space(), args);
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
    pub fn intern_or_known_symbol(&mut self, s: &str) -> Symbol {
        match s {
            "length" => Symbol::length(),
            "prototype" => Symbol::prototype(),
            "arguments" => Symbol::arguments(),
            "caller" => Symbol::caller(),
            "callee" => Symbol::callee(),
            "toString" => Symbol::toString(),
            "name" => Symbol::name(),
            "message" => Symbol::message(),
            "NaN" => Symbol::NaN(),
            "Infinity" => Symbol::Infinity(),
            "null" => Symbol::null(),
            "constructor" => Symbol::constructor(),
            "valueOf" => Symbol::valueOf(),
            "value" => Symbol::value(),
            "next" => Symbol::next(),
            "eval" => Symbol::eval(),
            "done" => Symbol::done(),
            "configrable" => Symbol::configurable(),
            "writable" => Symbol::writable(),
            "enumerable" => Symbol::enumerable(),
            "lastIndex" => Symbol::lastIndex(),
            "index" => Symbol::index(),
            "input" => Symbol::input(),
            "multiline" => Symbol::multiline(),

            "global" => Symbol::global(),
            "undefined" => Symbol::undefined(),
            "compare" => Symbol::compare(),
            "join" => Symbol::join(),
            "toPrimitive" => Symbol::toPrimitive(),
            _ => self.intern(s),
        }
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
    #[inline(always)]
    pub fn upop(&mut self) -> JsValue {
        unsafe {
            if self.stack == self.stack_start {
                panic!("Stack underflow");
            }
            self.stack = self.stack.sub(1);
            let val = self.stack.read();

            val
        }
    }
    #[inline(always)]
    pub fn upush(&mut self, val: JsValue) {
        unsafe {
            if self.stack == self.stack_end.add(1) {
                panic!("Stack overflow");
            }
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
        let space = Heap::new();
        let stack = memmap2::MmapMut::map_anon(8 * 1024 * 8).expect("failed to allocate stack");
        let ptr = stack.as_ptr() as *mut JsValue;

        let stack_end = unsafe { ptr.add(8 * 1024) };
        unsafe {
            let mut scan = ptr;
            while scan < stack_end {
                scan.write(JsValue::undefined());
                scan = scan.add(1);
            }
        }

        let mut this = VirtualMachineRef(Box::into_raw(Box::new(Self {
            space,
            stack: stack.as_ptr() as *mut _,
            stack_start: stack.as_ptr() as *mut _,
            stack_map: stack,
            options: opts,
            interner: SymbolTable::new(),
            global_data: Box::new(GlobalData::default()),
            global_object: None,
            thrown_error: JsValue::undefined(),
            return_value: JsValue::undefined(),

            frame: null_mut(),

            stacktrace: None,
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
                    let mut scan = vm.stack_start;
                    while scan < vm.stack.sub(1) {
                        let val = scan.read();
                        if !val.is_empty() {
                            val.trace(tracer);
                        }
                        scan = scan.add(1);
                    }
                }
            },
        ));
        this.space().defer_gc();

        this.global_data.empty_object_struct = Some(Structure::new_indexed(&mut this, None, false));
        let s = Structure::new_unique_indexed(&mut this, None, false);
        let mut proto = JsObject::new(&mut this, s, JsObject::get_class(), ObjectTag::Ordinary);
        this.global_data.object_prototype = Some(proto);
        this.global_data.function_struct = Some(Structure::new_indexed(&mut this, None, false));
        this.global_data.normal_arguments_structure =
            Some(Structure::new_indexed(&mut this, None, false));
        this.global_object = Some(JsGlobal::new(&mut this));
        this.global_data
            .empty_object_struct
            .unwrap()
            .change_prototype_with_no_transition(proto);

        let name = this.intern_or_known_symbol("Object");
        let mut obj_constructor = JsNativeFunction::new(&mut this, name, object_constructor, 1);
        let _ = obj_constructor.define_own_property(
            &mut this,
            Symbol::prototype(),
            &*DataDescriptor::new(JsValue::new(proto), NONE),
            false,
        );
        let _ = proto.define_own_property(
            &mut this,
            Symbol::constructor(),
            &*DataDescriptor::new(JsValue::new(obj_constructor), W | C),
            false,
        );
        let obj_to_string =
            JsNativeFunction::new(&mut this, Symbol::toString(), object_to_string, 0);
        let _ = proto.define_own_property(
            &mut this,
            Symbol::toString(),
            &*DataDescriptor::new(JsValue::new(obj_to_string), W | C),
            false,
        );
        let name = this.intern("Object");
        this.global_data
            .empty_object_struct
            .unwrap()
            .change_prototype_with_no_transition(proto);
        this.global_data.number_structure = Some(Structure::new_indexed(&mut this, None, false));
        this.init_error(proto);
        assert!(this.global_data().error_structure.is_some());
        let _ = this.global_object().define_own_property(
            &mut this,
            name,
            &*DataDescriptor::new(JsValue::new(obj_constructor), W | C),
            false,
        );
        this.init_array(proto);
        assert!(
            this.global_object()
                .get(&mut this, name)
                .unwrap_or_else(|_| panic!())
                .as_object()
                .cell
                == obj_constructor.cell
        );

        this.init_func(proto);
        this.space().undefer_gc();
        //this.space().gc();
        this
    }
    pub fn global_object(&self) -> Gc<JsObject> {
        unwrap_unchecked(self.global_object)
    }
    pub fn space(&mut self) -> &mut Heap {
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
    fn init_object(&mut self) {}
    fn init_func(&mut self, obj_proto: Gc<JsObject>) {
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let name = self.intern_or_known_symbol("Function");
        let mut func_proto = JsNativeFunction::new(self, name, function_prototype, 1);
        self.global_data
            .function_struct
            .unwrap()
            .change_prototype_with_no_transition(func_proto);
        self.global_data.func_prototype = Some(func_proto);
        let mut func_ctor = JsNativeFunction::new(self, name, function_prototype, 1);

        let _ = self
            .global_object()
            .put(self, name, JsValue::new(func_ctor), false);
        let s = func_proto
            .structure()
            .change_prototype_transition(self, Some(obj_proto));
        (*func_proto).set_structure(self, s);

        let _ = func_proto.define_own_property(
            self,
            Symbol::constructor(),
            &*DataDescriptor::new(JsValue::new(func_ctor), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, Symbol::toString(), function_bind, 0);
        let name = self.intern_or_known_symbol("bind");
        let _ = func_proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::new(f), W | C),
            false,
        );
        let f = JsNativeFunction::new(self, Symbol::toString(), function_to_string, 0);
        let _ = func_proto.define_own_property(
            self,
            Symbol::toString(),
            &*DataDescriptor::new(JsValue::new(f), W | C),
            false,
        );
    }
    fn init_array(&mut self, obj_proto: Gc<JsObject>) {
        let structure = Structure::new_indexed(self, None, true);
        self.global_data.array_structure = Some(structure);
        let structure = Structure::new_unique_indexed(self, Some(obj_proto), false);
        let mut proto = JsObject::new(self, structure, JsObject::get_class(), ObjectTag::Ordinary);
        self.global_data
            .array_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);
        let mut constructor = JsNativeFunction::new(self, Symbol::constructor(), array_ctor, 1);

        let name = self.intern("Array");
        let _ = self
            .global_object()
            .put(self, name, JsValue::new(constructor), false);

        let _ = constructor.define_own_property(
            self,
            Symbol::prototype(),
            &*DataDescriptor::new(JsValue::new(proto), NONE),
            false,
        );

        let name = self.intern("isArray");
        let is_array = JsNativeFunction::new(self, name, array_is_array, 1);
        let _ = constructor.put(self, name, JsValue::new(is_array), false);
        let name = self.intern("of");
        let array_of = JsNativeFunction::new(self, name, array_of, 1);
        let _ = constructor.put(self, name, JsValue::new(array_of), false);
        let name = self.intern("from");
        let array_from = JsNativeFunction::new(self, name, array_from, 1);
        let _ = constructor.put(self, name, JsValue::new(array_from), false);
        let _ = proto.define_own_property(
            self,
            Symbol::constructor(),
            &*DataDescriptor::new(JsValue::new(constructor), W | C),
            false,
        );
        let name = self.intern_or_known_symbol("join");
        let join = JsNativeFunction::new(self, name, array_join, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::new(join), W | C | E),
            false,
        );

        let name = self.intern_or_known_symbol("toString");
        let to_string = JsNativeFunction::new(self, name, array_join, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::new(to_string), W | C | E),
            false,
        );

        let name = self.intern_or_known_symbol("push");
        let push = JsNativeFunction::new(self, name, array_push, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::new(push), W | C | E),
            false,
        );
        let name = self.intern_or_known_symbol("pop");
        let pop = JsNativeFunction::new(self, name, array_pop, 1);
        let _ = proto.define_own_property(
            self,
            name,
            &*DataDescriptor::new(JsValue::new(pop), W | C | E),
            false,
        );
        self.global_data.array_prototype = Some(proto);
        let arr = self.intern("Array");
        let _ = self.global_object().define_own_property(
            self,
            arr,
            &*DataDescriptor::new(JsValue::new(constructor), W | C),
            false,
        );
    }
    fn init_error(&mut self, obj_proto: Gc<JsObject>) {
        self.global_data.error_structure = Some(Structure::new_indexed(self, None, false));
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

        let n = Symbol::name();
        let s = JsString::new(self, "Error");
        let e = JsString::new(self, "");
        let m = Symbol::message();
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
            &*DataDescriptor::new(JsValue::new(ctor), W | C),
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

            let n = Symbol::name();
            let s = JsString::new(self, "EvalError");
            let e = JsString::new(self, "");
            let m = Symbol::message();
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
                &*DataDescriptor::new(JsValue::new(sub_ctor), W | C),
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

            let n = Symbol::name();
            let s = JsString::new(self, "TypeError");
            let e = JsString::new(self, "");
            let m = Symbol::message();
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
                &*DataDescriptor::new(JsValue::new(sub_ctor), W | C),
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

            let n = Symbol::name();
            let s = JsString::new(self, "ReferenceError");
            let e = JsString::new(self, "");
            let m = Symbol::message();
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

        // range error
        {
            let structure = Structure::new_unique_with_proto(self, Some(proto), false);
            let mut sub_proto = JsObject::new(
                self,
                structure,
                JsReferenceError::get_class(),
                ObjectTag::Ordinary,
            );

            self.global_data
                .range_error_structure
                .unwrap()
                .change_prototype_with_no_transition(sub_proto);
            let sym = self.intern("RangeError");
            let mut sub_ctor = JsNativeFunction::new(self, sym, range_error_constructor, 1);
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

            let n = Symbol::name();
            let s = JsString::new(self, "RangeError");
            let e = JsString::new(self, "");
            let m = Symbol::message();
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

            self.global_data.range_error = Some(sub_proto);
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
    pub(crate) array_prototype: Option<Gc<JsObject>>,
    pub(crate) func_prototype: Option<Gc<JsObject>>,
    pub(crate) string_structure: Option<Gc<Structure>>,
    pub(crate) number_structure: Option<Gc<Structure>>,
    pub(crate) array_structure: Option<Gc<Structure>>,
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
            // let _ = Vec::from_raw_parts(self.stack_start, 0, 16 * 1024);
        }
    }
}

impl AsMut<Heap> for VirtualMachine {
    fn as_mut(&mut self) -> &mut Heap {
        &mut self.space
    }
}

impl AsMut<Heap> for VirtualMachineRef {
    fn as_mut(&mut self) -> &mut Heap {
        &mut self.space
    }
}
