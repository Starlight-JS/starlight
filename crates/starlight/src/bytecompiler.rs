/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::vm::{code_block::FileLocation, *};
use crate::{
    bytecode::{opcodes::Opcode, TypeFeedBack},
    prelude::*,
    vm::{code_block::CodeBlock, context::Context},
};
use std::convert::TryInto;
use std::u16;
use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};
use swc_common::{errors::Handler, sync::Lrc};
use swc_common::{FileName, SourceMap};
use swc_ecmascript::parser::*;
pub struct LoopControlInfo {
    breaks: Vec<Box<dyn FnOnce(&mut ByteCompiler)>>,
    continues: Vec<Box<dyn FnOnce(&mut ByteCompiler)>>,
}
use super::codegen::BindingKind;
use super::codegen::Scope as Analyzer;
use swc_common::DUMMY_SP;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use swc_ecmascript::visit::VisitWith;

use swc_ecmascript::{ast::*, visit::noop_visit_type};
pub type ScopeRef = Rc<RefCell<Scope>>;
/// JS variable scope representation at compile-time.
pub struct Scope {
    pub parent: Option<ScopeRef>,
    pub variables: HashMap<Symbol, Variable>,

    pub depth: u32,
}
impl Scope {
    pub fn add_var(&mut self, name: Symbol, ix: u16) -> u16 {
        self.variables.insert(
            name,
            Variable {
                kind: VariableKind::Var,
                name,
                index: ix,
                dont_free: false,
            },
        );
        ix
    }
    pub fn add_const_var(&mut self, name: Symbol, ix: u16) -> u16 {
        self.variables.insert(
            name,
            Variable {
                kind: VariableKind::Const,
                name,
                index: ix,
                dont_free: true,
            },
        );
        ix
    }

    pub fn add_let_var(&mut self, name: Symbol, ix: u16) -> u16 {
        self.variables.insert(
            name,
            Variable {
                kind: VariableKind::Let,
                name,
                index: ix,
                dont_free: true,
            },
        );
        ix
    }
}

pub struct Variable {
    pub name: Symbol,
    pub index: u16,
    pub kind: VariableKind,
    pub dont_free: bool,
}

pub enum VariableKind {
    Let,
    Const,
    Var,
    Global,
}
#[derive(Clone, Debug)]
pub enum Access {
    Variable(u16, u32),
    Global(Symbol),
    ById(Symbol),
    ArrayPat(Vec<(usize, Access)>),
    ByVal,
    This,
}

impl Access {
    pub fn expects_this(&self) -> bool {
        match self {
            Self::ById(_) => true,
            Self::ByVal => true,
            Self::ArrayPat(_) => true,
            _ => false,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Val {
    Float(u64),
    Str(String),
}

#[derive(Debug)]
pub enum CompileError {
    NotYetImpl(String),
}

pub struct ByteCompiler {
    pub builtins: bool,
    pub code: GcPointer<CodeBlock>,
    pub scope: Rc<RefCell<Scope>>,
    pub val_map: HashMap<Val, u32>,
    pub name_map: HashMap<Symbol, u32>,
    pub lci: Vec<LoopControlInfo>,
    pub fmap: HashMap<Symbol, u32>,
    pub top_level: bool,
    pub tail_pos: bool,
    /// Freelist of unused variable indexes
    pub variable_freelist: Vec<u32>,

    pub info: Option<Vec<(Range<usize>, FileLocation)>>,

    pub is_try: bool,
}

impl ByteCompiler {
    pub fn get_val(&mut self, ctx: GcPointer<Context>, val: Val) -> u32 {
        if let Some(ix) = self.val_map.get(&val) {
            return *ix;
        }

        let val_ = match val.clone() {
            Val::Float(x) => JsValue::new(f64::from_bits(x)),
            Val::Str(x) => JsValue::encode_object_value(JsString::new(ctx, x)),
        };
        let ix = self.code.literals.len();
        self.code.literals.push(val_);
        self.val_map.insert(val, ix as _);
        ix as _
    }
    pub fn get_val2(&mut self, val: JsValue) -> u32 {
        let ix = self.code.literals.len();
        self.code.literals.push(val);

        ix as _
    }
    pub fn get_sym(&mut self, name: Symbol) -> u32 {
        if let Some(ix) = self.name_map.get(&name) {
            return *ix;
        }
        let ix = self.code.names.len();
        self.code.names.push(name);
        self.name_map.insert(name, ix as _);
        ix as _
    }
    fn lookup_scope(&self, var: Symbol) -> Option<(u16, ScopeRef)> {
        let scope = self.scope.clone();

        if let Some(var) = scope.borrow().variables.get(&var).map(|x| x.index) {
            return Some((var, scope.clone()));
        }
        let mut scope = self.scope.borrow().parent.clone();
        while let Some(ns) = scope {
            if let Some(var) = ns.borrow().variables.get(&var).map(|x| x.index) {
                return Some((var, ns.clone()));
            }
            scope = ns.borrow().parent.clone();
        }
        None
    }

    fn access_var(&self, var: Symbol) -> Access {
        if let Some((ix, scope)) = self.lookup_scope(var) {
            let cur_depth = self.scope.borrow().depth;
            let depth = cur_depth - scope.borrow().depth;
            Access::Variable(ix, depth)
        } else {
            Access::Global(var)
        }
    }
    pub fn emit_get_local(&mut self, depth: u32, index: u32) {
        if depth == 0 {
            self.emit(Opcode::OP_GE0GL, &[index], false);
        } else {
            self.emit(Opcode::OP_GET_ENV, &[depth], false);
            self.emit(Opcode::OP_GET_LOCAL, &[index], false);
        }
    }
    pub fn emit_set_local(&mut self, depth: u32, index: u32) {
        if depth == 0 {
            self.emit(Opcode::OP_GE0SL, &[index], false);
        } else {
            self.emit(Opcode::OP_GET_ENV, &[depth], false);
            self.emit(Opcode::OP_SET_LOCAL, &[index], false);
        }
    }

    pub fn decl_const(&mut self, name: Symbol) -> u16 {
        if let Some((ix, scope)) = self.lookup_scope(name) {
            let cur_depth = self.scope.borrow().depth;
            let _depth = cur_depth - scope.borrow().depth;
            self.emit(Opcode::OP_DECL_CONST, &[ix as _], false);
            return ix;
        } else {
            unreachable!(
                "const '{}' not found",
                crate::vm::symbol_table::symbol_table().description(name.get_id())
            )
        }
    }

    pub fn create_const(&mut self, name: Symbol) -> u16 {
        let ix = if let Some(ix) = self.variable_freelist.pop() {
            self.scope.borrow_mut().add_const_var(name, ix as _);
            ix as u16
        } else {
            self.code.var_count += 1;
            self.scope
                .borrow_mut()
                .add_const_var(name, self.code.var_count as u16 - 1)
        };
        self.emit(Opcode::OP_DECL_CONST, &[ix as _], false);
        ix
    }

    pub fn decl_let(&mut self, name: Symbol) -> u16 {
        let ix = if let Some(ix) = self.variable_freelist.pop() {
            self.scope.borrow_mut().add_let_var(name, ix as _);
            ix as u16
        } else {
            self.code.var_count += 1;
            self.scope
                .borrow_mut()
                .add_let_var(name, self.code.var_count as u16 - 1)
        };
        self.emit(Opcode::OP_DECL_LET, &[ix as _], false);
        ix
    }

    pub fn ident_to_sym(id: &Ident) -> Symbol {
        let s: &str = &id.sym;
        s.intern()
    }
    pub fn var_decl(
        &mut self,
        ctx: GcPointer<Context>,
        var: &VarDecl,
        export: bool,
    ) -> Result<Vec<Symbol>, CompileError> {
        let mut names = vec![];
        for decl in var.decls.iter() {
            match &decl.name {
                Pat::Ident(name) => {
                    let name_ = Self::ident_to_sym(&name.id);
                    let ix = if VarDeclKind::Var == var.kind || VarDeclKind::Const == var.kind {
                        None
                    } else {
                        Some(if let Some(ix) = self.variable_freelist.pop() {
                            self.scope.borrow_mut().add_let_var(name_, ix as _);
                            ix as u16
                        } else {
                            self.code.var_count += 1;
                            self.scope
                                .borrow_mut()
                                .add_let_var(name_, self.code.var_count as u16 - 1)
                        })
                    };
                    match &decl.init {
                        Some(ref init) => {
                            self.expr(ctx, init, true, false)?;
                        }
                        None => {
                            self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        }
                    }
                    names.push(Self::ident_to_sym(&name.id));

                    match var.kind {
                        VarDeclKind::Const => {
                            //self.emit(Opcode::OP_DECL_CONST, &[ix.unwrap() as _], false);
                            self.decl_const(Self::ident_to_sym(&name.id));
                        }
                        VarDeclKind::Let => {
                            self.emit(Opcode::OP_DECL_LET, &[ix.unwrap() as _], false);
                        }
                        VarDeclKind::Var => {
                            let acc = self.access_var(Self::ident_to_sym(&name.id));
                            self.access_set(acc)?;
                        }
                    }

                    if export {
                        let var = self.access_var(name_);
                        self.access_get(var)?;
                        let module = self.access_var("@module".intern());
                        self.access_get(module)?;
                        let exports = self.get_sym("@exports".intern());
                        self.emit(Opcode::OP_GET_BY_ID, &[exports], true);
                        let sym = self.get_sym(name_);
                        self.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                    }
                }

                x => {
                    return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                }
            }
        }
        Ok(names)
    }
    pub fn access_delete(&mut self, acc: Access) {
        match acc {
            Access::Global(x) => {
                let id = self.get_sym(x);
                self.emit(Opcode::OP_GLOBALTHIS, &[], false);
                self.emit(Opcode::OP_DELETE_BY_ID, &[id], false);
            }
            Access::ByVal => {
                self.emit(Opcode::OP_DELETE_BY_VAL, &[], false);
            }
            Access::ById(x) => {
                let id = self.get_sym(x);
                self.emit(Opcode::OP_DELETE_BY_ID, &[id], false);
            }
            Access::Variable(_ix, _depth) => {
                self.emit(Opcode::OP_PUSH_TRUE, &[], false);
                // self.access_set()
            }
            Access::This => {
                self.emit(Opcode::OP_PUSH_THIS, &[], false);
            }
            _ => unreachable!(),
        }
    }
    pub fn access_set(&mut self, acc: Access) -> Result<(), CompileError> {
        match acc {
            Access::Variable(index, depth) => {
                /*  self.emit(Opcode::OP_GET_ENV, &[depth], false);
                self.emit(Opcode::OP_SET_LOCAL, &[index as _], false);*/
                self.emit_set_local(depth as _, index as _);
                //self.emit_u16(index);
            }
            Access::Global(x) => {
                let name = self.get_sym(x);
                self.emit(Opcode::OP_GLOBALTHIS, &[], false);
                self.emit(Opcode::OP_PUT_BY_ID, &[name], true);
            }
            Access::ById(name) => {
                let name = self.get_sym(name);
                self.emit(Opcode::OP_PUT_BY_ID, &[name], true);
            }
            Access::ByVal => self.emit(Opcode::OP_PUT_BY_VAL, &[0], false),
            Access::ArrayPat(x) => {
                // we expect object to be on stack
                for (_, acc) in x {
                    self.access_set(acc)?;
                }
            }
            x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
        }
        Ok(())
    }
    pub fn access_get(&mut self, acc: Access) -> Result<(), CompileError> {
        match acc {
            Access::Variable(index, depth) => {
                self.emit_get_local(depth as _, index as _);
            }
            Access::Global(x) => {
                let name = self.get_sym(x);
                self.emit(Opcode::OP_GLOBALTHIS, &[], false);
                if self.is_try {
                    self.emit(Opcode::OP_TRY_GET_BY_ID, &[name], true);
                } else {
                    self.emit(Opcode::OP_GET_BY_ID, &[name], true);
                }
            }
            Access::ById(name) => {
                let name = self.get_sym(name);
                self.emit(Opcode::OP_GET_BY_ID, &[name], true);
            }
            Access::ByVal => self.emit(Opcode::OP_GET_BY_VAL, &[0], false),
            Access::ArrayPat(acc) => {
                // we expect object to be on stack there.
                for (index, access) in acc {
                    self.emit(Opcode::OP_DUP, &[], false); // dup object to perform array index.
                    self.emit(Opcode::OP_PUSH_INT, &[index as i32 as u32], false);
                    self.emit(Opcode::OP_SWAP, &[], false);
                    self.emit(Opcode::OP_GET_BY_VAL, &[0], false);
                    self.access_get(access)?;
                }
            }
            x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
        }
        Ok(())
    }

    pub fn compile_access(
        &mut self,
        ctx: GcPointer<Context>,
        expr: &Expr,
        dup: bool,
    ) -> Result<Access, CompileError> {
        match expr {
            Expr::Ident(id) => Ok(self.access_var(Self::ident_to_sym(id))),
            Expr::Member(member) => {
                match &member.obj {
                    ExprOrSuper::Expr(e) => self.expr(ctx, e, true, false)?,
                    _ => return Err(CompileError::NotYetImpl("NYI: super access".to_string())),
                }
                if dup {
                    self.emit(Opcode::OP_DUP, &[], false);
                }
                let name = if member.computed {
                    None
                } else if let Expr::Ident(name) = &*member.prop {
                    Some(Self::ident_to_sym(name))
                } else {
                    None
                };
                if name.is_none() {
                    self.expr(ctx, &member.prop, true, false)?;
                    self.emit(Opcode::OP_SWAP, &[], false);
                }

                Ok(if let Some(name) = name {
                    Access::ById(name)
                } else {
                    Access::ByVal
                })
            }
            Expr::This(_) => Ok(Access::This),
            x => Err(CompileError::NotYetImpl(format!("NYI: Access {:?}", x))),
        }
    }
    pub fn finish(&mut self, ctx: GcPointer<Context>) -> GcPointer<CodeBlock> {
        if ctx.vm.options.dump_bytecode {
            let mut buf = String::new();
            let name = ctx.description(self.code.name);
            self.code.display_to(&mut buf).unwrap();
            eprintln!("Code block '{}' at {:p}: \n {}", name, self.code, buf);
        }
        self.code.literals_ptr = self.code.literals.as_ptr();
        self.code
    }
    pub fn compile_fn(
        &mut self,
        ctx: GcPointer<Context>,
        fun: &Function,
    ) -> Result<(), CompileError> {
        /*#[cfg(feature = "perf")]
        {
            self.vm.perf.set_prev_inst(crate::vm::perf::Perf::CODEGEN);
        }*/
        let is_strict = match fun.body {
            Some(ref body) => {
                if body.stmts.is_empty() {
                    false
                } else {
                    body.stmts[0].is_use_strict()
                }
            }
            None => false,
        };
        self.code.strict = is_strict;

        match fun.body {
            Some(ref body) => {
                self.compile(ctx, &body.stmts, false)?;
            }
            None => {}
        }
        self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
        self.emit(Opcode::OP_RET, &[], false);
        //self.finish(&mut self.vm);
        /*#[cfg(feature = "perf")]
        {
            self.vm.perf.get_perf(crate::vm::perf::Perf::INVALID);
        }*/
        Ok(())
    }
    pub fn compile_code(
        mut ctx: GcPointer<Context>,
        params_: &[String],
        rel_path: &str,
        body: String,
        builtins: bool,
    ) -> Result<JsValue, CompileError> {
        let mut params = vec![];
        let mut rat = None;
        let scope = Rc::new(RefCell::new(Scope {
            variables: HashMap::new(),
            parent: None,
            depth: 0,
        }));
        let mut code = CodeBlock::new(ctx, "<anonymous>".intern(), false, rel_path.into());
        let mut compiler = ByteCompiler {
            lci: Vec::new(),
            builtins,
            variable_freelist: Vec::with_capacity(4),
            code,
            tail_pos: false,
            info: None,
            fmap: HashMap::new(),
            val_map: HashMap::new(),
            name_map: HashMap::new(),
            top_level: false,
            scope,
            is_try: true,
        };
        let mut p = 0;
        for x in params_.iter() {
            params.push(x.intern());
            p += 1;
            compiler.scope.borrow_mut().add_var(x.intern(), p - 1);
        }
        code.param_count = params.len() as _;
        code.var_count = p as _;
        code.rest_at = rat;
        let cm: Lrc<SourceMap> = Default::default();
        let _e = BufferedError::default();

        let handler = Handler::with_emitter(true, false, Box::new(MyEmiter::default()));

        let fm = cm.new_source_file(FileName::Custom("<anonymous>".into()), body);

        let mut parser = Parser::new(Syntax::Es(init_es_config()), StringInput::from(&*fm), None);

        for e in parser.take_errors() {
            e.into_diagnostic(&handler).emit();
        }

        let script = match parser.parse_script() {
            Ok(script) => script,
            Err(e) => {
                return Err(CompileError::NotYetImpl(format!("{}", e.kind().msg())));
            }
        };

        let is_strict = if script.body.is_empty() {
            false
        } else {
            script.body[0].is_use_strict()
        };

        compiler.code.strict = is_strict;

        compiler.compile(ctx, &script.body, false)?;

        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
        compiler.emit(Opcode::OP_RET, &[], false);
        //compiler.compile(&script.body);
        let mut code = compiler.finish(ctx);

        //let mut code = ByteCompiler::compile_script(&mut *vmref, &script, path.to_owned());

        //code.display_to(&mut OutBuf).unwrap();

        let env = crate::vm::environment::Environment::new(ctx, 0);
        let fun = JsVMFunction::new(ctx, code, env);
        Ok(JsValue::new(fun))
    }
    pub fn function(
        &mut self,
        ctx: GcPointer<Context>,
        function: &Function,
        name: Symbol,
        expr: bool,
    ) -> Result<(), CompileError> {
        let mut _rest = None;
        let mut params = vec![];
        let mut rat = None;
        let (mut code, ix) = if !expr {
            (
                self.code.codes[self.fmap.get(&name).copied().unwrap() as usize],
                self.fmap.get(&name).copied().unwrap() as usize,
            )
        } else {
            let p = self.code.path.clone();
            let mut code = CodeBlock::new(ctx, name, false, p);
            self.code.codes.push(code);
            (code, self.code.codes.len() - 1)
        };
        if function.is_async {
            return Err(CompileError::NotYetImpl("NYI: async".to_string()));
        }
        code.is_generator = function.is_generator;
        let scope = Rc::new(RefCell::new(Scope {
            variables: HashMap::new(),
            parent: Some(self.scope.clone()),
            depth: self.scope.borrow().depth + 1,
        }));

        let mut compiler = ByteCompiler {
            lci: Vec::new(),
            builtins: self.builtins,
            variable_freelist: Vec::with_capacity(4),
            code,
            info: None,
            tail_pos: false,
            fmap: HashMap::new(),
            val_map: HashMap::new(),
            name_map: HashMap::new(),
            top_level: false,
            scope,
            is_try: true,
        };
        let mut p = 0;
        for x in function.params.iter() {
            match x.pat {
                Pat::Ident(ref x) => {
                    params.push(Self::ident_to_sym(&x.id));
                    p += 1;
                    compiler
                        .scope
                        .borrow_mut()
                        .add_var(Self::ident_to_sym(&x.id), p - 1);
                }
                Pat::Rest(ref r) => match &*r.arg {
                    Pat::Ident(ref id) => {
                        p += 1;
                        _rest = Some(Self::ident_to_sym(&id.id));
                        rat = Some(
                            compiler
                                .scope
                                .borrow_mut()
                                .add_var(Self::ident_to_sym(&id.id), p - 1)
                                as u32,
                        );
                    }
                    ref x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
                },
                ref x => {
                    return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                }
            }
        }

        code.param_count = params.len() as _;
        code.var_count = p as _;
        code.rest_at = rat;
        if code.is_generator {
            compiler.emit(Opcode::OP_INITIAL_YIELD, &[], false);
        }
        compiler.compile_fn(ctx, function)?;
        compiler.finish(ctx);

        let ix = if expr {
            ix as u32
        } else {
            *self.fmap.get(&name).unwrap()
        };
        self.emit(Opcode::OP_GET_FUNCTION, &[ix], false);
        Ok(())
    }
    pub fn fn_expr(
        &mut self,
        ctx: GcPointer<Context>,
        fun: &FnExpr,
        used: bool,
    ) -> Result<(), CompileError> {
        self.push_scope();
        let name = if let Some(ref id) = fun.ident {
            Self::ident_to_sym(id)
        } else {
            "<anonymous>".intern()
        };
        self.function(ctx, &fun.function, name, true)?;
        if name != "<anonymous>".intern() {
            self.emit(Opcode::OP_DUP, &[], false);
            let var = self.access_var(name);
            self.access_set(var)?;
        }
        self.pop_scope();
        if !used {
            self.emit(Opcode::OP_POP, &[], false);
        }
        Ok(())
    }

    pub fn analyze_module(
        &mut self,
        ctx: GcPointer<Context>,
        body: &[ModuleItem],
    ) -> Result<(), CompileError> {
        let scopea = Analyzer::analyze_module_items(body);
        for var in scopea.vars.iter() {
            match var.1.kind() {
                BindingKind::Const => {
                    let s: &str = &(var.0).0;
                    let name = s.intern();
                    let c = self.code.var_count;
                    self.scope.borrow_mut().add_const_var(name, c as _);
                    self.code.var_count += 1;
                }
                BindingKind::Function => {
                    let s: &str = &(var.0).0;
                    let name = s.intern();
                    let c = self.code.var_count;
                    self.scope.borrow_mut().add_let_var(name, c as _);
                    self.code.var_count += 1;
                }
                _ => (),
            }
        }

        let mut res = Ok(());
        VisitFnDecl::visit_module(body, &mut |decl| {
            let name = Self::ident_to_sym(&decl.ident);
            let p = self.code.path.clone();
            let mut code = CodeBlock::new(ctx, name, false, p);
            code.file_name = self.code.file_name.clone();
            let ix = self.code.codes.len();
            self.code.codes.push(code);
            self.fmap.insert(name, ix as _);
            let name = Self::ident_to_sym(&decl.ident);
            match self.function(ctx, &decl.function, Self::ident_to_sym(&decl.ident), false) {
                Ok(()) => {}
                Err(e) => res = Err(e),
            }
            // self.emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
            let var = self.access_var(name);
            self.access_set(var).unwrap_or_else(|_| panic!("wtf"));
        });
        res
    }

    pub fn compile_module(
        mut ctx: GcPointer<Context>,
        file: &str,
        path: &str,
        name: &str,
        module: &Module,
    ) -> Result<GcPointer<CodeBlock>, CompileError> {
        let name = name.intern();

        let mut code = CodeBlock::new(ctx, name, false, path.into());
        code.file_name = file.to_string();
        let mut compiler = ByteCompiler {
            lci: Vec::new(),
            top_level: true,
            info: None,
            tail_pos: false,
            builtins: false,
            scope: Rc::new(RefCell::new(Scope {
                parent: None,
                variables: Default::default(),
                depth: 0,
            })),
            variable_freelist: vec![],
            code,
            val_map: Default::default(),
            name_map: Default::default(),
            fmap: Default::default(),
            is_try: true,
        };
        code.var_count = 1;
        code.param_count = 1;
        compiler.scope.borrow_mut().add_var("@module".intern(), 0);

        let loader = JsValue::new(ctx.module_loader().unwrap());

        let loader_val = compiler.get_val2(loader);
        compiler.analyze_module(ctx, &module.body)?;

        if let Some(item) = module.body.get(0) {
            match item {
                ModuleItem::Stmt(stmt) => match stmt {
                    Stmt::Expr(e) => match &*e.expr {
                        Expr::Lit(x) => match x {
                            Lit::Str(x) => {
                                code.strict = x.value.to_string() == "use strict";
                            }
                            _ => (),
                        },
                        _ => (),
                    },
                    _ => (),
                },
                _ => (),
            }
        }
        for item in &module.body {
            match item {
                ModuleItem::Stmt(stmt) => {
                    compiler.stmt(ctx, stmt)?;
                }
                ModuleItem::ModuleDecl(module_decl) => match module_decl {
                    ModuleDecl::Import(import) => {
                        let src = compiler.get_val(ctx, Val::Str(import.src.value.to_string()));
                        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        compiler.emit(Opcode::OP_PUSH_LITERAL, &[loader_val], false);
                        compiler.emit(Opcode::OP_PUSH_LITERAL, &[src], false);
                        compiler.emit(Opcode::OP_CALL, &[1], false);
                        for specifier in import.specifiers.iter() {
                            match specifier {
                                ImportSpecifier::Default(default) => {
                                    compiler.emit(Opcode::OP_DUP, &[], false);
                                    let default = Self::ident_to_sym(&default.local);
                                    let sym = compiler.get_sym("@default".intern());
                                    compiler.emit(Opcode::OP_TRY_GET_BY_ID, &[sym], true);
                                    compiler.create_const(default);
                                }
                                ImportSpecifier::Namespace(namespace) => {
                                    compiler.emit(Opcode::OP_DUP, &[], false);
                                    let default = Self::ident_to_sym(&namespace.local);
                                    compiler.create_const(default);
                                }
                                ImportSpecifier::Named(named) => {
                                    compiler.emit(Opcode::OP_DUP, &[], false);
                                    let import_as = match named.imported {
                                        Some(ref name) => Self::ident_to_sym(name),
                                        None => Self::ident_to_sym(&named.local),
                                    };
                                    let name = Self::ident_to_sym(&named.local);
                                    let sym = compiler.get_sym("@exports".intern());
                                    compiler.emit(Opcode::OP_GET_BY_ID, &[sym], true);
                                    let sym = compiler.get_sym(name);
                                    compiler.emit(Opcode::OP_GET_BY_ID, &[sym], true);
                                    compiler.create_const(import_as);
                                }
                            }
                        }
                        compiler.emit(Opcode::OP_POP, &[], false);
                    }
                    ModuleDecl::ExportDecl(decl) => {
                        compiler.decl(ctx, &decl.decl, true)?;
                    }
                    ModuleDecl::ExportDefaultDecl(decl) => {
                        match decl.decl {
                            DefaultDecl::Fn(ref fun) => {
                                compiler.fn_expr(ctx, fun, true)?;
                            }
                            ref x => {
                                return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                            }
                        }

                        let module = compiler.access_var("@module".intern());
                        compiler.access_get(module)?;
                        let default = compiler.get_sym("@default".intern());
                        compiler.emit(Opcode::OP_PUT_BY_ID, &[default], true);
                    }
                    ModuleDecl::ExportDefaultExpr(expr) => {
                        compiler.expr(ctx, &expr.expr, true, false)?;
                        let module = compiler.access_var("@module".intern());
                        compiler.access_get(module)?;
                        let default = compiler.get_sym("@default".intern());
                        compiler.emit(Opcode::OP_PUT_BY_ID, &[default], true);
                    }
                    ModuleDecl::ExportNamed(named_export) => {
                        if named_export.src.is_some() {
                            return Err(CompileError::NotYetImpl(
                                "NYI: export * from mod".to_string(),
                            ));
                        }

                        for specifier in named_export.specifiers.iter() {
                            match specifier {
                                ExportSpecifier::Named(named) => {
                                    let export_as = match named.exported {
                                        Some(ref exported) => Self::ident_to_sym(exported),
                                        None => Self::ident_to_sym(&named.orig),
                                    };
                                    let orig = compiler.access_var(Self::ident_to_sym(&named.orig));
                                    compiler.access_get(orig)?;
                                    let module = compiler.access_var("@module".intern());
                                    compiler.access_get(module)?;
                                    let exports = compiler.get_sym("@exports".intern());
                                    compiler.emit(Opcode::OP_GET_BY_ID, &[exports], true);
                                    let sym = compiler.get_sym(export_as);
                                    compiler.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                                }
                                _ => {
                                    return Err(CompileError::NotYetImpl(format!(
                                        "NYI: {:?}",
                                        specifier
                                    )));
                                }
                            }
                        }
                    }
                    x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
                },
            }
        }
        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
        compiler.emit(Opcode::OP_RET, &[], false);
        let result = compiler.finish(ctx);

        Ok(result)
    }
    pub fn compile_script(
        mut ctx: GcPointer<Context>,
        p: &Script,
        path: &str,
        fname: String,
        builtins: bool,
    ) -> Result<GcPointer<CodeBlock>, CompileError> {
        let name = "<script>".intern();
        let mut code = CodeBlock::new(ctx, name, false, path.into());
        code.file_name = fname;
        let mut compiler = ByteCompiler {
            lci: Vec::new(),
            top_level: true,
            info: None,
            tail_pos: false,
            builtins,
            scope: Rc::new(RefCell::new(Scope {
                parent: None,
                variables: Default::default(),
                depth: 0,
            })),
            variable_freelist: vec![],
            code,
            val_map: Default::default(),
            name_map: Default::default(),
            fmap: Default::default(),
            is_try: true,
        };

        let is_strict = match p.body.get(0) {
            Some(body) => body.is_use_strict(),
            None => false,
        };
        code.top_level = true;
        code.strict = is_strict;
        compiler.push_scope();
        compiler.compile(ctx, &p.body, false)?;
        compiler.pop_scope();
        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
        compiler.emit(Opcode::OP_RET, &[], false);
        let result = compiler.finish(ctx);

        Ok(result)
    }

    pub fn compile_eval(
        mut ctx: GcPointer<Context>,
        p: &Script,
        path: &str,
        fname: String,
        builtins: bool,
    ) -> Result<GcPointer<CodeBlock>, CompileError> {
        let name = "<script>".intern();
        let mut code = CodeBlock::new(ctx, name, false, path.into());
        code.file_name = fname;
        let mut compiler = ByteCompiler {
            lci: Vec::new(),
            top_level: true,
            info: None,
            tail_pos: false,
            builtins,
            scope: Rc::new(RefCell::new(Scope {
                parent: None,
                variables: Default::default(),
                depth: 0,
            })),
            variable_freelist: vec![],
            code,
            val_map: Default::default(),
            name_map: Default::default(),
            fmap: Default::default(),
            is_try: true,
        };

        let is_strict = match p.body.get(0) {
            Some(body) => body.is_use_strict(),
            None => false,
        };
        code.top_level = true;
        code.strict = is_strict;
        compiler.push_scope();
        compiler.compile(ctx, &p.body, true)?;
        compiler.pop_scope();
        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
        compiler.emit(Opcode::OP_RET, &[], false);
        let result = compiler.finish(ctx);

        Ok(result)
    }

    pub fn analyze(&mut self, ctx: GcPointer<Context>, body: &[Stmt]) -> Result<(), CompileError> {
        let scopea = Analyzer::analyze_stmts(body);

        for var in scopea.vars.iter() {
            match var.1.kind() {
                BindingKind::Var if !self.top_level => {
                    let s: &str = &(var.0).0;
                    let name = s.intern();
                    let c = self.code.var_count;
                    self.scope.borrow_mut().add_var(name, c as _);
                    self.code.var_count += 1;
                }
                BindingKind::Function if !self.top_level => {
                    let s: &str = &(var.0).0;
                    let name = s.intern();
                    let c = self.code.var_count;
                    self.scope.borrow_mut().add_var(name, c as _);
                    self.code.var_count += 1;
                }
                BindingKind::Const => {
                    let s: &str = &(var.0).0;
                    let name = s.intern();
                    let c = self.code.var_count;
                    self.scope.borrow_mut().add_const_var(name, c as _);
                    self.code.var_count += 1;
                }
                _ => {}
            }
        }
        let mut res = Ok(());
        VisitFnDecl::visit(body, &mut |decl: &FnDecl| {
            let name = Self::ident_to_sym(&decl.ident);
            let p = self.code.path.clone();
            let mut code = CodeBlock::new(ctx, name, false, p);
            code.file_name = self.code.file_name.clone();
            let ix = self.code.codes.len();
            self.code.codes.push(code);
            self.fmap.insert(name, ix as _);
            let name = Self::ident_to_sym(&decl.ident);

            match self.function(ctx, &decl.function, Self::ident_to_sym(&decl.ident), false) {
                Ok(()) => {}
                Err(e) => res = Err(e),
            }

            // self.emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
            let var = self.access_var(name);
            self.access_set(var).unwrap_or_else(|_| panic!("wtf"));
        });

        for stmt in body.iter() {
            if contains_ident(stmt, "arguments") {
                self.code.use_arguments = true;
                let c = self.code.var_count;
                self.code.args_at = self
                    .scope
                    .borrow_mut()
                    .add_var("arguments".intern(), c as _) as _;
                break;
            }
        }
        res
    }

    pub fn compile(
        &mut self,
        ctx: GcPointer<Context>,
        body: &[Stmt],
        _last_val_ret: bool,
    ) -> Result<(), CompileError> {
        self.analyze(ctx, body)?;
        for (index, stmt) in body.iter().enumerate() {
            if index == body.len() - 1 && _last_val_ret {
                if let Stmt::Expr(ref expr) = stmt {
                    self.expr(ctx, &expr.expr, true, false)?;
                    self.emit(Opcode::OP_RET, &[], false);
                    break;
                }
            }

            self.stmt(ctx, stmt)?;
        }
        Ok(())
    }

    /// Push scope and return current scope depth
    pub fn push_scope(&mut self) -> u32 {
        let d = self.scope.borrow().depth;
        let new_scope = Rc::new(RefCell::new(Scope {
            parent: Some(self.scope.clone()),
            depth: self.scope.borrow().depth,
            variables: Default::default(),
        }));
        self.scope = new_scope;
        d
    }
    pub fn pop_scope(&mut self) {
        let scope = self.scope.clone();
        self.scope = scope.borrow().parent.clone().expect("No scopes left");
        for var in scope.borrow().variables.iter() {
            if !var.1.dont_free {
                self.variable_freelist.push(var.1.index as u32);
            }
        }
    }
    pub fn push_lci(&mut self, _continue_target: u32, _depth: u32) {
        self.lci.push(LoopControlInfo {
            continues: vec![],
            breaks: vec![],
        });
    }

    pub fn pop_lci(&mut self) {
        let mut lci = self.lci.pop().unwrap();
        while let Some(break_) = lci.breaks.pop() {
            break_(self);
        }
    }
    pub fn decl(
        &mut self,
        ctx: GcPointer<Context>,
        decl: &Decl,
        export: bool,
    ) -> Result<(), CompileError> {
        match decl {
            Decl::Var(var) => {
                self.var_decl(ctx, var, export)?;
            }

            Decl::Fn(fun) => {
                let name = Self::ident_to_sym(&fun.ident);
                if export {
                    let var = self.access_var(name);
                    self.access_get(var)?;
                    let module = self.access_var("@module".intern());
                    self.access_get(module)?;
                    let exports = self.get_sym("@exports".intern());
                    self.emit(Opcode::OP_GET_BY_ID, &[exports], true);
                    let sym = self.get_sym(name);
                    self.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                }
            }

            x => {
                return Err(CompileError::NotYetImpl(format!("NYI Decl: {:?}", x)));
            }
        }
        Ok(())
    }
    pub fn stmt(&mut self, ctx: GcPointer<Context>, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Switch(switch) => {
                let d = self.scope.borrow().depth;
                self.push_lci(0, d);
                self.expr(ctx, &switch.discriminant, true, false)?;

                let mut last_jump: Option<Box<dyn FnOnce(&mut ByteCompiler)>> = None;

                for case in switch.cases.iter() {
                    match case.test {
                        Some(ref expr) => {
                            self.emit(Opcode::OP_DUP, &[], false);
                            self.expr(ctx, expr, true, false)?;
                            self.emit(Opcode::OP_EQ, &[], false);
                            let fail = self.cjmp(false);
                            match last_jump {
                                None => {}
                                Some(jmp) => {
                                    jmp(self);
                                }
                            }
                            for stmt in case.cons.iter() {
                                self.stmt(ctx, stmt)?;
                            }
                            last_jump = Some(Box::new(self.jmp()));

                            fail(self);
                        }
                        None => {
                            for stmt in case.cons.iter() {
                                self.stmt(ctx, stmt)?;
                            }
                        }
                    }
                }
                self.pop_lci();
                self.emit(Opcode::OP_POP, &[], false);
            }
            Stmt::Expr(expr) => {
                self.expr(ctx, &expr.expr, false, false)?;
            }
            Stmt::Block(block) => {
                let _prev = self.push_scope();
                self.analyze(ctx, &block.stmts)?;
                // self.emit(Opcode::OP_PUSH_ENV, &[], false);
                for stmt in block.stmts.iter() {
                    self.stmt(ctx, stmt)?;
                }
                self.pop_scope();
                //self.emit(Opcode::OP_POP_ENV, &[], false);
                //self.emit(Opcode::OP_SET_ENV, &[prev], false);
            }
            Stmt::Return(ret) => {
                self.tail_pos = true;
                match ret.arg {
                    Some(ref arg) => self.expr(ctx, arg, true, true)?,
                    None => self.emit(Opcode::OP_PUSH_UNDEF, &[], false),
                };
                self.tail_pos = false;
                self.emit(Opcode::OP_RET, &[], false);
            }
            Stmt::Break(_) => {
                let br = self.jmp();
                self.lci.last_mut().unwrap().breaks.push(Box::new(br));
            }
            Stmt::Continue(_) => {
                let j = self.jmp();
                self.lci.last_mut().unwrap().continues.push(Box::new(j));
            }
            Stmt::ForIn(for_in) => {
                let depth = self.push_scope();

                self.analyze(ctx, &[Stmt::ForIn(for_in.clone())])?;

                // self.emit(Opcode::OP_PUSH_ENV, &[], false);
                let name = match for_in.left {
                    VarDeclOrPat::VarDecl(ref var_decl) => self.var_decl(ctx, var_decl, false)?[0],
                    VarDeclOrPat::Pat(Pat::Ident(ref ident)) => {
                        let sym = Self::ident_to_sym(&ident.id);
                        self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        self.emit(Opcode::OP_GET_ENV, &[0], false);
                        self.decl_let(sym);
                        sym
                    }
                    ref x => return Err(CompileError::NotYetImpl(format!("{:?}", x))),
                };

                self.expr(ctx, &for_in.right, true, false)?;
                let for_in_setup = self.jmp_custom(Opcode::OP_FORIN_SETUP);
                let head = self.code.code.len();
                self.push_lci(head as _, depth);
                let for_in_enumerate = self.jmp_custom(Opcode::OP_FORIN_ENUMERATE);
                let acc = self.access_var(name);
                self.access_set(acc)?;
                //self.emit(Opcode::OP_SET_LOCAL, &[name], true);
                self.stmt(ctx, &for_in.body)?;
                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }

                self.goto(head as _);

                for_in_enumerate(self);
                for_in_setup(self);

                // self.emit(Opcode::OP_POP_ENV, &[], false);
                self.pop_scope();
                self.emit(Opcode::OP_FORIN_LEAVE, &[], false);
                self.pop_lci();
            }
            Stmt::ForOf(for_of) => {
                let depth = self.push_scope();
                // self.emit(Opcode::OP_PUSH_ENV, &[], false);
                self.analyze(ctx, &[Stmt::ForOf(for_of.clone())])?;

                let name = match for_of.left {
                    VarDeclOrPat::VarDecl(ref var_decl) => self.var_decl(ctx, var_decl, false)?[0],
                    VarDeclOrPat::Pat(Pat::Ident(ref ident)) => {
                        let sym = Self::ident_to_sym(&ident.id);
                        self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        self.emit(Opcode::OP_GET_ENV, &[0], false);
                        self.decl_let(sym);
                        sym
                    }
                    ref x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
                };
                let iterator_id = "Symbol.iterator".intern().private();
                let iterator = self.get_sym(iterator_id);
                let next = self.get_sym("next".intern());
                let done = self.get_sym("done".intern());
                let value = self.get_sym("value".intern());
                self.expr(ctx, &for_of.right, true, false)?;
                self.emit(Opcode::OP_DUP, &[], false);
                self.emit(Opcode::OP_GET_BY_ID, &[iterator], true);
                self.emit(Opcode::OP_CALL, &[0], false);

                let head = self.code.code.len();
                self.push_lci(head as _, depth);
                // iterator is on stack, dup it twice to invoke `next` on it.
                self.emit(Opcode::OP_DUP, &[], false);
                self.emit(Opcode::OP_DUP, &[], false);
                self.emit(Opcode::OP_GET_BY_ID, &[next], true);
                self.emit(Opcode::OP_CALL, &[0], false);
                self.emit(Opcode::OP_DUP, &[], false);
                self.emit(Opcode::OP_GET_BY_ID, &[done], true);
                let end = self.cjmp(true);
                self.emit(Opcode::OP_GET_BY_ID, &[value], true);
                let acc = self.access_var(name);
                self.access_set(acc)?;
                self.stmt(ctx, &for_of.body)?;
                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }

                self.goto(head as _);

                end(self);
                self.pop_scope();
                self.emit(Opcode::OP_POP, &[], false);
                self.pop_lci();
            }
            Stmt::For(for_stmt) => {
                let _env = self.push_scope();
                // self.emit(Opcode::OP_PUSH_ENV, &[], false);
                match for_stmt.init {
                    Some(ref init) => match init {
                        VarDeclOrExpr::Expr(ref e) => {
                            self.expr(ctx, e, false, false)?;
                        }
                        VarDeclOrExpr::VarDecl(ref decl) => {
                            self.var_decl(ctx, decl, false)?;
                        }
                    },
                    None => {}
                }

                let head = self.code.code.len();
                self.push_lci(head as _, _env);
                match for_stmt.test {
                    Some(ref test) => {
                        self.expr(ctx, &**test, true, false)?;
                    }
                    None => {
                        self.emit(Opcode::OP_PUSH_TRUE, &[], false);
                    }
                }
                let jend = self.cjmp(false);
                self.stmt(ctx, &for_stmt.body)?;
                //let skip = self.jmp();
                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }

                //self.emit(Opcode::OP_POP_ENV, &[], false);
                //skip(self);
                if let Some(fin) = &for_stmt.update {
                    self.expr(ctx, &**fin, false, false)?;
                }
                self.goto(head as _);
                self.pop_lci();
                self.pop_scope();
                // self.emit(Opcode::OP_POP_ENV, &[], false);
                jend(self);

                //                self.emit(Opcode::OP_POP_ENV, &[], false);
            }
            Stmt::If(if_stmt) => {
                self.expr(ctx, &if_stmt.test, true, false)?;
                let jelse = self.cjmp(false);
                self.stmt(ctx, &if_stmt.cons)?;
                match if_stmt.alt {
                    None => {
                        jelse(self);
                    }
                    Some(ref alt) => {
                        let jend = self.jmp();
                        jelse(self);
                        self.stmt(ctx, &**alt)?;
                        jend(self);
                    }
                }
            }
            Stmt::Decl(decl) => self.decl(ctx, decl, false)?,
            Stmt::Empty(_) => {}
            Stmt::Throw(throw) => {
                self.expr(ctx, &throw.arg, true, false)?;
                self.emit(Opcode::OP_THROW, &[], false);
            }
            Stmt::Try(try_stmt) => {
                let try_push = self.try_();

                for stmt in try_stmt.block.stmts.iter() {
                    self.stmt(ctx, stmt)?;
                }
                self.emit(Opcode::OP_POP_CATCH, &[], false);
                let jfinally = self.jmp();
                try_push(self);
                let jcatch_finally = match try_stmt.handler {
                    Some(ref catch) => {
                        self.push_scope();

                        match catch.param {
                            Some(ref pat) => {
                                let acc = self.compile_access_pat(ctx, pat, false)?;
                                self.access_set(acc)?;
                            }
                            None => {
                                self.emit(Opcode::OP_POP, &[], false);
                            }
                        }
                        for stmt in catch.body.stmts.iter() {
                            self.stmt(ctx, stmt)?;
                        }
                        self.pop_scope();
                        self.jmp()
                    }
                    None => {
                        self.emit(Opcode::OP_POP, &[], false);
                        self.jmp()
                    }
                };

                jfinally(self);
                jcatch_finally(self);
                match try_stmt.finalizer {
                    Some(ref block) => {
                        self.push_scope();

                        for stmt in block.stmts.iter() {
                            self.stmt(ctx, stmt)?;
                        }

                        self.pop_scope();
                    }
                    None => {}
                }
            }
            Stmt::While(while_stmt) => {
                let head = self.code.code.len();
                let d = self.scope.borrow().depth;
                self.push_lci(head as _, d);
                self.expr(ctx, &while_stmt.test, true, false)?;
                let jend = self.cjmp(false);
                self.stmt(ctx, &while_stmt.body)?;

                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }
                self.goto(head);
                jend(self);
                self.pop_lci();
            }
            Stmt::DoWhile(do_while_stmt) => {
                let body = self.code.code.len();
                let d = self.scope.borrow().depth;
                self.push_lci(body as _, d);
                self.stmt(ctx, &do_while_stmt.body)?;

                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }
                self.expr(ctx, &do_while_stmt.test, true, false)?;
                let jend = self.cjmp(false);
                self.goto(body);
                jend(self);
                self.pop_lci();
            }
            x => {
                return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
            }
        }
        Ok(())
    }

    pub fn compile_pat_decl(&mut self, pat: &Pat) -> Result<(), CompileError> {
        match pat {
            Pat::Array(pat) => {
                for pat in pat.elems.iter() {
                    match pat {
                        Some(pat) => self.compile_pat_decl(pat)?,
                        _ => (),
                    }
                }
            }
            Pat::Ident(x) => {
                self.decl_let(Self::ident_to_sym(&x.id));
            }
            Pat::Object(object) => {
                for case in object.props.iter() {
                    match case {
                        ObjectPatProp::KeyValue(ref keyvalue) => match keyvalue.key {
                            PropName::Ident(ref id) => {
                                self.decl_let(Self::ident_to_sym(id));
                            }
                            PropName::Str(ref x) => {
                                self.decl_let(x.value.intern());
                            }
                            _ => (),
                        },
                        ObjectPatProp::Assign(x) => {
                            self.decl_let(Self::ident_to_sym(&x.key));
                        }
                        ObjectPatProp::Rest(x) => {
                            self.compile_pat_decl(&x.arg)?;
                        }
                    }
                }
            }
            Pat::Rest(x) => {
                self.compile_pat_decl(&x.arg)?;
            }
            Pat::Assign(x) => {
                self.compile_pat_decl(&x.left)?;
            }
            x => {
                return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
            }
        }
        Ok(())
    }
    pub fn compile_access_pat(
        &mut self,
        ctx: GcPointer<Context>,
        pat: &Pat,
        dup: bool,
    ) -> Result<Access, CompileError> {
        match pat {
            Pat::Ident(id) => Ok(self.access_var(Self::ident_to_sym(&id.id))),
            Pat::Expr(expr) => self.compile_access(ctx, expr, dup),
            Pat::Array(array) => {
                let mut acc = vec![];
                for (index, pat) in array.elems.iter().enumerate() {
                    match pat {
                        Some(pat) => {
                            let access = self.compile_access_pat(ctx, pat, false);
                            acc.push((index, access));
                        }
                        _ => (),
                    }
                }
                Err(CompileError::NotYetImpl(format!(
                    "NYI: Array access: {:?}",
                    array
                )))
            }
            x => Err(CompileError::NotYetImpl(format!("NYI:  {:?}", x))),
        }
    }

    pub fn expr(
        &mut self,
        ctx: GcPointer<Context>,
        expr: &Expr,
        used: bool,
        tail: bool,
    ) -> Result<(), CompileError> {
        match expr {
            Expr::Yield(yield_expr) => {
                if yield_expr.delegate {
                    return Err(CompileError::NotYetImpl("NYI: yield*".to_string()));
                }
                match yield_expr.arg {
                    Some(ref expr) => {
                        self.expr(ctx, &**expr, true, false)?;
                    }
                    None => {
                        self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                    }
                }
                self.emit(Opcode::OP_YIELD, &[], false);
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Ident(id) => {
                // TODO: When builtins are compiled we should add `___` prefix support for builtin symbols.
                // for example `___iterator` should become `"Symbol.iterator".intern().private()"` and as incle PUSH_LITERAL opcode.
                if &id.sym == "undefined" {
                    self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                } else {
                    let var = self.access_var(Self::ident_to_sym(id));
                    self.access_get(var)?;
                }
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Lit(lit) => {
                match lit {
                    Lit::Bool(x) => {
                        if x.value {
                            self.emit(Opcode::OP_PUSH_TRUE, &[], false)
                        } else {
                            self.emit(Opcode::OP_PUSH_FALSE, &[], false)
                        }
                    }
                    Lit::Null(_) => self.emit(Opcode::OP_PUSH_NULL, &[], false),
                    Lit::Num(num) => {
                        if num.value as i32 as f64 == num.value {
                            self.emit(Opcode::OP_PUSH_INT, &[num.value as i32 as u32], false)
                        } else {
                            let ix = self.get_val(ctx, Val::Float(num.value.to_bits()));
                            self.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                        }
                    }
                    Lit::Str(str) => {
                        let str = self.get_val(ctx, Val::Str(str.value.to_string()));
                        self.emit(Opcode::OP_PUSH_LITERAL, &[str], false);
                    }
                    Lit::Regex(regex) => {
                        let exp = regex.exp.to_string();
                        let flags = regex.flags.to_string();
                        let exp = JsString::new(ctx, exp);
                        let flags = JsString::new(ctx, flags);
                        let mut args = [JsValue::new(exp), JsValue::new(flags)];
                        let args = Arguments::new(JsValue::encode_undefined_value(), &mut args);
                        let regexp =
                            crate::jsrt::regexp::regexp_constructor(ctx, &args).map_err(|e| {
                                CompileError::NotYetImpl(format!(
                                    "Regexp constructor error {}",
                                    e.to_string(ctx).unwrap()
                                ))
                            })?;
                        let val = self.get_val2(regexp);
                        self.emit(Opcode::OP_PUSH_LITERAL, &[val], false);
                    }
                    Lit::BigInt(_) => {
                        return Err(CompileError::NotYetImpl(
                            "Unimplemented JS literal: BigInt".to_string(),
                        ))
                    }
                    Lit::JSXText(_) => {
                        return Err(CompileError::NotYetImpl(
                            "Unimplemented JS literal: JSXText".to_string(),
                        ))
                    } //todo!("{:?}", x),
                    #[allow(unreachable_patterns)]
                    x => {
                        return Err(CompileError::NotYetImpl(format!(
                            "Unimplemented JS literal: {:?}",
                            x
                        )))
                    }
                }
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::This(_) => {
                if used {
                    self.emit(Opcode::OP_PUSH_THIS, &[], false);
                }
            }
            Expr::Member(_) => {
                let acc = self.compile_access(ctx, expr, false)?;
                self.access_get(acc)?;
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Object(object_lit) => {
                self.emit(Opcode::OP_NEWOBJECT, &[], false);
                for prop in object_lit.props.iter() {
                    match prop {
                        PropOrSpread::Prop(prop) => match &**prop {
                            Prop::Shorthand(ident) => {
                                self.emit(Opcode::OP_DUP, &[], false);
                                let ix = Self::ident_to_sym(ident);
                                let acc = self.access_var(ix);
                                let sym = self.get_sym(ix);
                                self.access_get(acc)?;
                                // self.emit(Opcode::OP_GET_LOCAL, &[sym], true);
                                self.emit(Opcode::OP_SWAP, &[], false);
                                self.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                            }
                            Prop::KeyValue(assign) => {
                                self.emit(Opcode::OP_DUP, &[], false);
                                self.expr(ctx, &assign.value, true, false)?;
                                match assign.key {
                                    PropName::Ident(ref id) => {
                                        let ix = Self::ident_to_sym(id);
                                        let sym = self.get_sym(ix);
                                        self.emit(Opcode::OP_SWAP, &[], false);
                                        self.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                                    }
                                    PropName::Str(ref s) => {
                                        let ix = self.get_val(ctx, Val::Str(s.value.to_string()));
                                        self.emit(Opcode::OP_SWAP, &[], false);
                                        self.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                                        self.emit(Opcode::OP_SWAP, &[], false);
                                        self.emit(Opcode::OP_PUT_BY_VAL, &[0], false);
                                    }
                                    PropName::Num(n) => {
                                        let val = n.value;
                                        if val as i32 as f64 == val {
                                            self.emit(Opcode::OP_SWAP, &[], false);
                                            self.emit(
                                                Opcode::OP_PUSH_INT,
                                                &[val as i32 as u32],
                                                false,
                                            );
                                            self.emit(Opcode::OP_SWAP, &[], false);
                                            self.emit(Opcode::OP_PUT_BY_VAL, &[0], false);
                                        } else {
                                            let ix = self.get_val(ctx, Val::Float(val.to_bits()));
                                            self.emit(Opcode::OP_SWAP, &[], false);
                                            self.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                                            self.emit(Opcode::OP_SWAP, &[], false);
                                            self.emit(Opcode::OP_PUT_BY_VAL, &[0], false);
                                        }
                                    }
                                    ref x => {
                                        return Err(CompileError::NotYetImpl(format!(
                                            "NYI: {:?}",
                                            x
                                        )));
                                    }
                                }
                            }
                            p => {
                                return Err(CompileError::NotYetImpl(format!("NYI: {:?}", p)));
                            }
                        },
                        x => {
                            return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                        }
                    }
                }
            }
            x if is_builtin_call(x, self.builtins) => {
                if let Expr::Call(call) = x {
                    self.handle_builtin_call(ctx, call)?;
                }
            }
            x if is_codegen_plugin_call(ctx, x, self.builtins) => {
                if let Expr::Call(call) = x {
                    self.handle_codegen_plugin_call(ctx, call)?;
                }
            }
            Expr::Call(call) if !is_builtin_call(expr, self.builtins) => {
                match call.callee {
                    ExprOrSuper::Super(_) => {
                        return Err(CompileError::NotYetImpl("NYI: super call".to_string()));
                    } // todo super call
                    ExprOrSuper::Expr(ref expr) => match &**expr {
                        Expr::Member(member) => {
                            let name = if let Expr::Ident(id) = &*member.prop {
                                let s: &str = &id.sym;
                                let name = s.intern();
                                Some(self.get_sym(name))
                            } else {
                                self.expr(ctx, &member.prop, true, false)?;
                                None
                            };
                            match member.obj {
                                ExprOrSuper::Expr(ref expr) => {
                                    self.expr(ctx, expr, true, false)?;
                                    if name.is_some() {
                                        self.emit(Opcode::OP_DUP, &[], false);
                                    }
                                }
                                ExprOrSuper::Super(_super) => {
                                    return Err(CompileError::NotYetImpl(
                                        "NYI: super call".to_string(),
                                    ));
                                }
                            }
                            if let Some(name) = name {
                                self.emit(Opcode::OP_GET_BY_ID, &[name], true);
                            } else {
                                self.emit(Opcode::OP_GET_BY_VAL_PUSH_OBJ, &[0], false);
                            }
                        }
                        _ => {
                            self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                            self.expr(ctx, &**expr, true, false)?;
                        }
                    },
                }
                // self.emit(Opcode::OP_PUSH_EMPTY, &[], false);
                let has_spread = call.args.iter().any(|x| x.spread.is_some());
                if has_spread {
                    for arg in call.args.iter().rev() {
                        self.expr(ctx, &arg.expr, true, false)?;
                        if arg.spread.is_some() {
                            self.emit(Opcode::OP_SPREAD, &[], false);
                        }
                    }
                    self.emit(Opcode::OP_NEWARRAY, &[call.args.len() as u32], false);
                } else {
                    for arg in call.args.iter() {
                        self.expr(ctx, &arg.expr, true, false)?;
                        assert!(arg.spread.is_none());
                    }
                }

                if !has_spread {
                    let op = if tail {
                        Opcode::OP_TAILCALL
                    } else {
                        Opcode::OP_CALL
                    };
                    self.emit(op, &[call.args.len() as u32], false);
                } else {
                    self.emit(
                        Opcode::OP_CALL_BUILTIN,
                        &[call.args.len() as _, 0, 0],
                        false,
                    );
                }
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Unary(unary) => {
                if let UnaryOp::Delete = unary.op {
                    let acc = self.compile_access(ctx, &*unary.arg, false)?;
                    self.access_delete(acc);
                    if !used {
                        self.emit(Opcode::OP_POP, &[], false)
                    }
                    return Ok(());
                }

                let mut is_type_of = false;
                if let UnaryOp::TypeOf = unary.op {
                    is_type_of = true;
                    self.is_try = false;
                }

                self.expr(ctx, &unary.arg, true, false)?;

                if is_type_of {
                    self.is_try = true;
                }

                match unary.op {
                    UnaryOp::Minus => self.emit(Opcode::OP_NEG, &[], false),
                    UnaryOp::Plus => self.emit(Opcode::OP_POS, &[], false),
                    UnaryOp::Tilde => self.emit(Opcode::OP_NOT, &[], false),
                    UnaryOp::Bang => self.emit(Opcode::OP_LOGICAL_NOT, &[], false),
                    UnaryOp::TypeOf => self.emit(Opcode::OP_TYPEOF, &[], false),
                    UnaryOp::Void => {
                        self.emit(Opcode::OP_POP, &[], false);
                        self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                    }
                    x => {
                        return Err(CompileError::NotYetImpl(format!("NYI Unary Op: {:?}", x)));
                    }
                }
                if !used {
                    self.emit(Opcode::OP_POP, &[], false)
                }
            }
            Expr::Update(update) => {
                let op = match update.op {
                    UpdateOp::PlusPlus => Opcode::OP_ADD,
                    UpdateOp::MinusMinus => Opcode::OP_SUB,
                };
                if update.prefix {
                    self.expr(ctx, &update.arg, true, false)?;
                    self.emit(Opcode::OP_PUSH_INT, &[1i32 as u32], false);
                    self.emit(op, &[0], false);
                    if used {
                        self.emit(Opcode::OP_DUP, &[], false);
                    }
                    let acc = self.compile_access(ctx, &update.arg, false)?;
                    self.access_set(acc)?;
                    //self.emit_store_expr(&update.arg);
                } else {
                    self.expr(ctx, &update.arg, true, false)?;
                    if used {
                        self.emit(Opcode::OP_DUP, &[], false);
                    }
                    self.emit(Opcode::OP_PUSH_INT, &[1i32 as u32], false);
                    self.emit(op, &[0], false);
                    if op == Opcode::OP_SUB {
                        self.emit(Opcode::OP_NEG, &[], false);
                    }
                    let acc = self.compile_access(ctx, &update.arg, false)?;
                    self.access_set(acc)?;
                    //self.emit_store_expr(&update.arg);
                }
            }
            Expr::New(call) => {
                self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                self.expr(ctx, &*call.callee, true, false)?;
                let argc = call.args.as_ref().map(|x| x.len() as u32).unwrap_or(0);
                let has_spread = if let Some(ref args) = call.args {
                    args.iter().any(|x| x.spread.is_some())
                } else {
                    false
                };
                if let Some(ref args) = call.args {
                    if has_spread {
                        for arg in args.iter().rev() {
                            self.expr(ctx, &arg.expr, true, false)?;
                            if arg.spread.is_some() {
                                self.emit(Opcode::OP_SPREAD, &[], false);
                            }
                        }
                        self.emit(Opcode::OP_NEWARRAY, &[argc], false);
                    } else {
                        for arg in args.iter() {
                            self.expr(ctx, &arg.expr, true, false)?;
                            assert!(arg.spread.is_none());
                        }
                    }
                }

                if !has_spread {
                    let op = if tail {
                        Opcode::OP_TAILNEW
                    } else {
                        Opcode::OP_NEW
                    };
                    self.emit(op, &[argc], false);
                } else {
                    self.emit(Opcode::OP_CALL_BUILTIN, &[argc as _, 0, 1], false);
                }
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Assign(assign) => {
                if let AssignOp::Assign = assign.op {
                    self.expr(ctx, &assign.right, true, false)?;
                    if used {
                        self.emit(Opcode::OP_DUP, &[], false);
                    }
                    let acc = match &assign.left {
                        PatOrExpr::Expr(expr) => self.compile_access(ctx, expr, false)?,
                        PatOrExpr::Pat(p) => self.compile_access_pat(ctx, p, false)?,
                    };

                    self.access_set(acc)?;
                } else {
                    self.expr(ctx, &assign.right, true, false)?;
                    let left = match &assign.left {
                        PatOrExpr::Expr(e) => self.compile_access(ctx, e, false)?,
                        PatOrExpr::Pat(p) => self.compile_access_pat(ctx, p, false)?,
                    };
                    self.access_get(left)?;

                    let op = match assign.op {
                        AssignOp::AddAssign => Opcode::OP_ADD,
                        AssignOp::SubAssign => Opcode::OP_SUB,
                        AssignOp::MulAssign => Opcode::OP_MUL,
                        AssignOp::DivAssign => Opcode::OP_DIV,
                        AssignOp::BitAndAssign => Opcode::OP_AND,
                        AssignOp::BitOrAssign => Opcode::OP_OR,
                        AssignOp::BitXorAssign => Opcode::OP_XOR,
                        AssignOp::ModAssign => Opcode::OP_REM,
                        AssignOp::RShiftAssign => Opcode::OP_SHR,
                        AssignOp::LShiftAssign => Opcode::OP_SHL,
                        x => {
                            return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                        }
                    };
                    let additional: &'static [u32] = if op == Opcode::OP_ADD
                        || op == Opcode::OP_MUL
                        || op == Opcode::OP_REM
                        || op == Opcode::OP_SUB
                        || op == Opcode::OP_DIV
                    {
                        &[0u32]
                    } else {
                        &[]
                    };
                    self.emit(op, additional, false);
                    if used {
                        self.emit(Opcode::OP_DUP, &[], false);
                    }
                    let left = match &assign.left {
                        PatOrExpr::Expr(e) => self.compile_access(ctx, e, false)?,
                        PatOrExpr::Pat(p) => self.compile_access_pat(ctx, p, false)?,
                    };
                    self.access_set(left)?;
                }
            }
            Expr::Bin(binary) => {
                match binary.op {
                    BinaryOp::LogicalOr => {
                        self.expr(ctx, &binary.left, true, false)?;
                        self.emit(Opcode::OP_DUP, &[], false);
                        let jtrue = self.cjmp(true);
                        self.emit(Opcode::OP_POP, &[], false);
                        self.expr(ctx, &binary.right, true, false)?;
                        //let end = self.jmp();
                        jtrue(self);
                        // self.emit(Opcode::OP_PUSH_TRUE, &[], false);
                        //end(self);
                        if !used {
                            self.emit(Opcode::OP_POP, &[], false);
                        }
                        return Ok(());
                    }
                    BinaryOp::LogicalAnd => {
                        self.expr(ctx, &binary.left, true, false)?;
                        self.emit(Opcode::OP_DUP, &[], false);
                        let jfalse = self.cjmp(false);
                        self.emit(Opcode::OP_POP, &[], false);
                        self.expr(ctx, &binary.right, true, false)?;
                        let end = self.jmp();
                        jfalse(self);
                        end(self);
                        if !used {
                            self.emit(Opcode::OP_POP, &[], false);
                        }
                        return Ok(());
                    }

                    _ => (),
                }
                self.expr(ctx, &binary.right, true, false)?;
                self.expr(ctx, &binary.left, true, false)?;

                match binary.op {
                    BinaryOp::Add => {
                        self.emit(Opcode::OP_ADD, &[0], false);
                    }
                    BinaryOp::Sub => {
                        self.emit(Opcode::OP_SUB, &[0], false);
                    }
                    BinaryOp::Mul => {
                        self.emit(Opcode::OP_MUL, &[0], false);
                    }
                    BinaryOp::Div => {
                        self.emit(Opcode::OP_DIV, &[0], false);
                    }
                    BinaryOp::Mod => self.emit(Opcode::OP_REM, &[0], false),
                    BinaryOp::BitAnd => self.emit(Opcode::OP_AND, &[], false),
                    BinaryOp::BitOr => self.emit(Opcode::OP_OR, &[], false),
                    BinaryOp::BitXor => self.emit(Opcode::OP_XOR, &[], false),
                    BinaryOp::LShift => self.emit(Opcode::OP_SHL, &[], false),
                    BinaryOp::RShift => self.emit(Opcode::OP_SHR, &[], false),
                    BinaryOp::ZeroFillRShift => self.emit(Opcode::OP_USHR, &[], false),
                    BinaryOp::EqEq => {
                        self.emit(Opcode::OP_EQ, &[], false);
                    }
                    BinaryOp::EqEqEq => self.emit(Opcode::OP_STRICTEQ, &[], false),
                    BinaryOp::NotEq => self.emit(Opcode::OP_NEQ, &[], false),
                    BinaryOp::NotEqEq => self.emit(Opcode::OP_NSTRICTEQ, &[], false),
                    BinaryOp::Gt => self.emit(Opcode::OP_GREATER, &[], false),
                    BinaryOp::GtEq => self.emit(Opcode::OP_GREATEREQ, &[], false),
                    BinaryOp::Lt => self.emit(Opcode::OP_LESS, &[], false),
                    BinaryOp::LtEq => self.emit(Opcode::OP_LESSEQ, &[], false),
                    BinaryOp::In => self.emit(Opcode::OP_IN, &[], false),
                    BinaryOp::InstanceOf => self.emit(Opcode::OP_INSTANCEOF, &[], false),
                    x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
                }

                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Arrow(fun) => {
                let is_strict = match &fun.body {
                    BlockStmtOrExpr::BlockStmt(block) => {
                        if block.stmts.is_empty() {
                            false
                        } else {
                            block.stmts[0].is_use_strict()
                        }
                    }
                    _ => false,
                };
                let name = "<anonymous>".intern();
                let p = self.code.path.clone();
                let mut code = CodeBlock::new(ctx, name, false, p);
                code.file_name = self.code.file_name.clone();
                let mut compiler = ByteCompiler {
                    lci: Vec::new(),
                    top_level: false,
                    tail_pos: false,
                    builtins: self.builtins,
                    code,
                    variable_freelist: vec![],
                    val_map: Default::default(),
                    name_map: Default::default(),
                    info: None,
                    fmap: Default::default(),
                    scope: Rc::new(RefCell::new(Scope {
                        parent: Some(self.scope.clone()),
                        depth: self.scope.borrow().depth + 1,
                        variables: HashMap::new(),
                    })),
                    is_try: true,
                };
                code.strict = is_strict;
                let mut params = vec![];
                let mut rest_at = None;
                let mut p = 0;
                for x in fun.params.iter() {
                    match x {
                        Pat::Ident(ref x) => {
                            params.push(Self::ident_to_sym(&x.id));
                            p += 1;
                            compiler
                                .scope
                                .borrow_mut()
                                .add_var(Self::ident_to_sym(&x.id), p - 1);
                        }
                        Pat::Rest(ref r) => match &*r.arg {
                            Pat::Ident(ref id) => {
                                p += 1;
                                rest_at = Some(
                                    compiler
                                        .scope
                                        .borrow_mut()
                                        .add_var(Self::ident_to_sym(&id.id), p - 1)
                                        as u32,
                                );
                            }
                            ref x => return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x))),
                        },
                        x => {
                            return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
                        }
                    }
                }
                code.rest_at = rest_at;
                code.param_count = params.len() as _;
                code.var_count = p as _;
                match &fun.body {
                    BlockStmtOrExpr::BlockStmt(block) => {
                        compiler.compile(ctx, &block.stmts, false)?;
                        compiler.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        compiler.emit(Opcode::OP_RET, &[], false);
                    }
                    BlockStmtOrExpr::Expr(expr) => {
                        compiler.expr(ctx, expr, true, true)?;
                        compiler.emit(Opcode::OP_RET, &[], false);
                    }
                }
                let code = compiler.finish(ctx);
                let ix = self.code.codes.len();
                self.code.codes.push(code);
                let _nix = self.get_sym(name);
                self.emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
            }
            Expr::Seq(seq) => {
                let mut last = seq.exprs.len() - 1;
                for (i, expr) in seq.exprs.iter().enumerate() {
                    self.expr(ctx, expr, used && (last == i), tail)?;
                }
            }
            Expr::Fn(fun) => {
                self.fn_expr(ctx, fun, used)?;
            }

            Expr::Array(array_lit) => {
                for expr in array_lit.elems.iter().rev() {
                    match expr {
                        Some(expr) => {
                            self.expr(ctx, &expr.expr, true, false)?;
                            if expr.spread.is_some() {
                                self.emit(Opcode::OP_SPREAD, &[], false);
                            }
                        }
                        None => self.emit(Opcode::OP_PUSH_UNDEF, &[], false),
                    }
                }
                self.emit(Opcode::OP_NEWARRAY, &[array_lit.elems.len() as u32], false);
                if !used {
                    self.emit(Opcode::OP_POP, &[], false);
                }
            }

            Expr::Cond(cond) => {
                self.expr(ctx, &cond.test, true, false)?;
                let jelse = self.cjmp(false);
                self.expr(ctx, &cond.cons, used, tail)?;

                let jend = self.jmp();
                jelse(self);
                self.expr(ctx, &cond.alt, used, tail)?;
                jend(self);
            }
            Expr::Paren(p) => {
                self.expr(ctx, &p.expr, used, false)?;
            }
            x => {
                return Err(CompileError::NotYetImpl(format!("NYI: {:?}", x)));
            }
        }
        Ok(())
    }

    pub fn try_(&mut self) -> impl FnOnce(&mut Self) {
        let p = self.code.code.len();
        self.emit(Opcode::OP_PUSH_CATCH, &[0], false);

        move |this: &mut Self| {
            let to = this.code.code.len() - (p + 5);
            let ins = Opcode::OP_PUSH_CATCH;
            let bytes = (to as u32).to_le_bytes();
            this.code.code[p] = ins as u8;
            this.code.code[p + 1] = bytes[0];
            this.code.code[p + 2] = bytes[1];
            this.code.code[p + 3] = bytes[2];
            this.code.code[p + 4] = bytes[3];
        }
    }
    pub fn cjmp(&mut self, cond: bool) -> impl FnOnce(&mut Self) {
        let p = self.code.code.len();
        self.emit(Opcode::OP_JMP, &[0], false);

        move |this: &mut Self| {
            //  this.emit(Opcode::OP_NOP, &[], false);
            let to = this.code.code.len() - (p + 5);
            let ins = if cond {
                Opcode::OP_JMP_IF_TRUE
            } else {
                Opcode::OP_JMP_IF_FALSE
            };
            let bytes = (to as u32).to_le_bytes();
            this.code.code[p] = ins as u8;
            this.code.code[p + 1] = bytes[0];
            this.code.code[p + 2] = bytes[1];
            this.code.code[p + 3] = bytes[2];
            this.code.code[p + 4] = bytes[3];
        }
    }
    pub fn goto(&mut self, to: usize) {
        let at = self.code.code.len() as i32 + 5;
        self.emit(Opcode::OP_JMP, &[(to as i32 - at) as u32], false);
    }
    pub fn jmp(&mut self) -> impl FnOnce(&mut Self) {
        let p = self.code.code.len();
        self.emit(Opcode::OP_JMP, &[0], false);

        move |this: &mut Self| {
            // this.emit(Opcode::OP_NOP, &[], false);
            let to = this.code.code.len() - (p + 5);
            let bytes = (to as u32).to_le_bytes();
            this.code.code[p] = Opcode::OP_JMP as u8;
            this.code.code[p + 1] = bytes[0];
            this.code.code[p + 2] = bytes[1];
            this.code.code[p + 3] = bytes[2];
            this.code.code[p + 4] = bytes[3];
            //this.code.code[p] = ins as u8;
        }
    }

    pub fn jmp_custom(&mut self, op: Opcode) -> impl FnOnce(&mut Self) {
        let p = self.code.code.len();
        self.emit(op, &[0], false);

        move |this: &mut Self| {
            // this.emit(Opcode::OP_NOP, &[], false);
            let to = this.code.code.len() - (p + 5);
            let bytes = (to as u32).to_le_bytes();
            this.code.code[p] = op as u8;
            this.code.code[p + 1] = bytes[0];
            this.code.code[p + 2] = bytes[1];
            this.code.code[p + 3] = bytes[2];
            this.code.code[p + 4] = bytes[3];
            //this.code.code[p] = ins as u8;
        }
    }
    // fn declare_variable(&mut self,decl: &VarDecl) -> Vec<u32>
    pub fn emit(&mut self, op: Opcode, operands: &[u32], add_feedback: bool) {
        self.code.code.push(op as u8);
        for operand in operands.iter() {
            for x in operand.to_le_bytes().iter() {
                self.code.code.push(*x);
            }
        }
        if add_feedback {
            let f_ix = self.code.feedback.len() as u32;
            self.code.feedback.push(TypeFeedBack::None);
            for x in f_ix.to_le_bytes().iter() {
                self.code.code.push(*x);
            }
        }
    }

    pub fn emit_u8(&mut self, x: u8) {
        self.code.code.push(x);
    }

    pub fn emit_u16(&mut self, x: u16) {
        let bytes = x.to_le_bytes();
        self.code.code.extend(&bytes);
    }

    pub fn emit_u32(&mut self, x: u32) {
        self.code.code.extend(&x.to_le_bytes());
    }
}

impl<'a> VisitFnDecl<'a> {
    pub fn visit(stmts: &[Stmt], clos: &'a mut dyn FnMut(&FnDecl)) {
        let mut visit = Self { cb: clos };
        for stmt in stmts.iter() {
            stmt.visit_with(&Invalid { span: DUMMY_SP }, &mut visit)
        }
    }
    pub fn visit_module(body: &[ModuleItem], clos: &'a mut dyn FnMut(&FnDecl)) {
        let mut visit = Self { cb: clos };
        for item in body {
            /*match item {
                ModuleItem::ModuleDecl(decl) => match decl {
                    ModuleDecl::ExportDecl(decl) => decl
                        .decl
                        .visit_with(&Invalid { span: DUMMY_SP }, &mut visit),
                    ModuleDecl::ExportDefaultDecl(decl) => {
                        decl
                    }
                },
            }*/
            item.visit_with(&Invalid { span: DUMMY_SP }, &mut visit);
        }
    }
}

pub struct VisitFnDecl<'a> {
    cb: &'a mut dyn FnMut(&FnDecl),
}

impl<'a> Visit for VisitFnDecl<'a> {
    fn visit_fn_decl(&mut self, n: &FnDecl, _: &dyn Node) {
        (self.cb)(n);
    }
    fn visit_block_stmt(&mut self, _n: &BlockStmt, _: &dyn Node) {
        return;
    }
}
pub trait IsDirective {
    fn as_ref(&self) -> Option<&Stmt>;
    fn is_use_strict(&self) -> bool {
        match self.as_ref() {
            Some(&Stmt::Expr(ref expr)) => match *expr.expr {
                Expr::Lit(Lit::Str(Str {
                    ref value,
                    has_escape: false,
                    ..
                })) => value == "use strict",
                _ => false,
            },
            _ => false,
        }
    }
}

impl IsDirective for Stmt {
    fn as_ref(&self) -> Option<&Stmt> {
        Some(self)
    }
}

pub fn contains_ident<'a, N>(body: &N, ident: &'a str) -> bool
where
    N: VisitWith<IdentFinder<'a>>,
{
    let mut visitor = IdentFinder {
        found: false,
        ident,
    };
    body.visit_with(&Invalid { span: DUMMY_SP } as _, &mut visitor);
    visitor.found
}
pub struct IdentFinder<'a> {
    ident: &'a str,
    found: bool,
}

impl Visit for IdentFinder<'_> {
    noop_visit_type!();

    fn visit_expr(&mut self, e: &Expr, _: &dyn Node) {
        e.visit_children_with(self);

        match *e {
            Expr::Ident(ref i) if &i.sym == self.ident => {
                self.found = true;
            }
            _ => {}
        }
    }
}

fn is_codegen_plugin_call(ctx: GcPointer<Context>, e: &Expr, builtins: bool) -> bool {
    if !builtins && !ctx.vm.options.codegen_plugins {
        return false;
    }
    if let Expr::Call(call) = e {
        if let ExprOrSuper::Expr(expr) = &call.callee {
            match &**expr {
                // ___foo(x,y)
                Expr::Ident(x) => {
                    let str = &*x.sym;
                    return ctx.vm.codegen_plugins.contains_key(str);
                }
                _ => {
                    return false;
                }
            }
        }
    }
    false
}

fn is_builtin_call(e: &Expr, builtin_compilation: bool) -> bool {
    if !builtin_compilation {
        return false;
    }
    if let Expr::Call(call) = e {
        if let ExprOrSuper::Expr(expr) = &call.callee {
            match &**expr {
                // ___foo(x,y)
                Expr::Ident(x) => {
                    let str = &*x.sym;
                    return str.starts_with("___");
                }
                // foo.___call(x,y)
                // now first support foo.___call
                Expr::Member(m) => {
                    if let Expr::Ident(x) = &*m.prop {
                        let str = &*x.sym;
                        return str == "___call";
                    }
                }
                _ => {
                    return false;
                }
            }
        }
    }
    false
}
impl ByteCompiler {
    pub fn handle_codegen_plugin_call(
        &mut self,
        ctx: GcPointer<Context>,
        call: &CallExpr,
    ) -> Result<(), CompileError> {
        let plugin_name = if let ExprOrSuper::Expr(expr) = &call.callee {
            if let Expr::Ident(x) = &**expr {
                &*x.sym
            } else {
                return Err(CompileError::NotYetImpl(
                    "Incorrect codegen plugin syntax".to_string(),
                ));
            }
        } else {
            return Err(CompileError::NotYetImpl(
                "Incorrect codegen plugin syntax".to_string(),
            ));
        };
        let vm = ctx.vm;
        let plugin = vm.codegen_plugins.get(plugin_name).unwrap();
        plugin(self, ctx, &call.args)
    }

    /// TODO List:
    /// - Implement  `___call` ,`___tailcall`.
    /// - Getters for special symbols. Should be expanded to PUSH_LITERAL.
    pub fn handle_builtin_call(
        &mut self,
        ctx: GcPointer<Context>,
        call: &CallExpr,
    ) -> Result<(), CompileError> {
        let (member, builtin_call_name) = if let ExprOrSuper::Expr(expr) = &call.callee {
            match &**expr {
                // ___foo(x,y)
                Expr::Ident(x) => {
                    let str = &*x.sym;
                    (None, str.to_string())
                }
                // foo.___call(x,y)
                // now first support foo.___call
                Expr::Member(m) => {
                    if let Expr::Ident(x) = &*m.prop {
                        let str = &*x.sym;
                        assert!(str == "___call");
                        (Some(&m.obj), str.to_string())
                    } else {
                        unreachable!()
                    }
                }
                _ => {
                    unreachable!()
                }
            }
        } else {
            unreachable!()
        };
        let nstr: &str = &builtin_call_name;

        match nstr {
            "___toObject" => {
                if let Some(msg) = call.args.get(1) {
                    self.expr(ctx, &msg.expr, true, false)?;
                } else {
                    self.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                }
                self.expr(ctx, &call.args[0].expr, true, false)?;

                self.emit(Opcode::OP_TO_OBJECT, &[], false);
            }

            "___toLength" => {
                self.expr(ctx, &call.args[0].expr, true, false)?;

                self.emit(Opcode::OP_TO_LENGTH, &[], false);
            }
            "___toIntegerOrInfinity" => {
                self.expr(ctx, &call.args[0].expr, true, false)?;

                self.emit(Opcode::OP_TO_INTEGER_OR_INFINITY, &[], false);
            }
            "___isCallable" => {
                self.expr(ctx, &call.args[0].expr, true, false)?;

                self.emit(Opcode::OP_IS_CALLABLE, &[], false);
            }
            "___isObject" => {
                self.expr(ctx, &call.args[0].expr, true, false)?;
                self.emit(Opcode::OP_IS_OBJECT, &[], false);
            }
            "___isConstructor" => {
                self.expr(ctx, &call.args[0].expr, true, false)?;

                self.emit(Opcode::OP_IS_CTOR, &[], false);
            }
            "___call" => {
                if let Some(func) = &member {
                    if let ExprOrSuper::Expr(x) = &func {
                        if let Expr::Ident(_) = &**x {
                            self.expr(ctx, &call.args[0].expr, true, false)?;
                            self.expr(ctx, &**x, true, false)?;
                            for i in 1..call.args.len() {
                                self.expr(ctx, &call.args[i].expr, true, false)?;
                            }
                            let operands: u32 = (call.args.len() - 1).try_into().unwrap();
                            self.emit(Opcode::OP_CALL, &[operands], false);
                        } else {
                            todo!()
                        }
                    } else {
                        todo!()
                    }
                } else {
                    unreachable!()
                }
            }
            _ => todo!("{}", nstr),
        }
        Ok(())
    }
}
