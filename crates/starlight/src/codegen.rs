use crate::{
    bytecode::{opcodes::Opcode, TypeFeedBack},
    gc::cell::{GcPointer, Trace, Tracer},
    vm::code_block::CodeBlock,
    vm::symbol_table::*,
    vm::{string::JsString, symbol_table::Symbol, value::*, Runtime, RuntimeRef},
};
use std::collections::HashMap;
use swc_atoms::JsWord;
use swc_common::DUMMY_SP;
use swc_ecmascript::utils::find_ids;
use swc_ecmascript::utils::ident::IdentLike;
use swc_ecmascript::utils::Id;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use swc_ecmascript::visit::VisitWith;
use swc_ecmascript::{ast::*, visit::noop_visit_type};

pub struct LoopControlInfo {
    breaks: Vec<Box<dyn FnOnce(&mut Compiler)>>,
    continues: Vec<Box<dyn FnOnce(&mut Compiler)>>,
    scope_depth: i32,
}
pub struct Compiler {
    builder: ByteCodeBuilder,
    vm: RuntimeRef,
    top_level: bool,
    lci: Vec<LoopControlInfo>,
    fmap: HashMap<Symbol, u32>,
}

pub struct ByteCodeBuilder {
    pub code: GcPointer<CodeBlock>,
    pub name_map: HashMap<Symbol, u32>,
    pub val_map: HashMap<Val, u32>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Val {
    Float(u64),
    Str(String),
}
impl ByteCodeBuilder {
    pub fn finish(&mut self, vm: &mut Runtime) -> GcPointer<CodeBlock> {
        if vm.options.dump_bytecode {
            let mut buf = String::new();
            let name = vm.description(self.code.name);
            self.code.display_to(&mut buf).unwrap();
            eprintln!("Code block '{}' at {:p}: \n {}", name, self.code, buf);
        }
        self.code
    }
    pub fn new(vm: &mut Runtime, name: Symbol, params: &[Symbol], strict: bool) -> Self {
        let mut this = Self {
            code: CodeBlock::new(vm, name, strict),
            val_map: Default::default(),
            name_map: Default::default(),
        };
        this.code.params = params.to_vec();
        this
    }
    pub fn get_val(&mut self, vm: &mut Runtime, val: Val) -> u32 {
        if let Some(ix) = self.val_map.get(&val) {
            return *ix;
        }

        let val_ = match val.clone() {
            Val::Float(x) => JsValue::encode_f64_value(f64::from_bits(x)),
            Val::Str(x) => JsValue::encode_object_value(JsString::new(vm, x)),
        };
        let ix = self.code.literals.len();
        self.code.literals.push(val_);
        self.val_map.insert(val, ix as _);
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
}
unsafe impl Trace for ByteCodeBuilder {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.code.trace(tracer);
    }
}

#[derive(Debug)]
pub struct Scope {
    pub vars: HashMap<Id, Var>,
    symbols: HashMap<JsWord, Vec<Id>>,
}
impl<'a> VisitFnDecl<'a> {
    pub fn visit(stmts: &[Stmt], clos: &'a mut dyn FnMut(&FnDecl)) {
        let mut visit = Self { cb: clos };
        for stmt in stmts.iter() {
            stmt.visit_with(&Invalid { span: DUMMY_SP }, &mut visit)
        }
    }
}
impl Scope {
    pub fn analyze_stmts(stmts: &[Stmt]) -> Self {
        let mut scope = Self {
            vars: Default::default(),
            symbols: Default::default(),
        };
        let mut path = vec![];

        for stmt in stmts {
            stmt.visit_with(
                &Invalid { span: DUMMY_SP },
                &mut Analyzer {
                    scope: &mut scope,
                    path: &mut path,
                },
            );
        }
        scope
    }
    pub fn analyze(program: &Program) -> Self {
        let mut scope = Self {
            vars: Default::default(),
            symbols: Default::default(),
        };
        let mut path = vec![];

        program.visit_with(
            &Invalid { span: DUMMY_SP },
            &mut Analyzer {
                scope: &mut scope,
                path: &mut path,
            },
        );

        scope
    }

    // Get all declarations with a symbol.
    #[allow(dead_code)]
    pub fn ids_with_symbol(&self, sym: &JsWord) -> Option<&Vec<Id>> {
        self.symbols.get(sym)
    }

    pub fn var(&self, id: &Id) -> Option<&Var> {
        self.vars.get(id)
    }
}

#[derive(Debug)]
pub struct Var {
    path: Vec<ScopeKind>,
    kind: BindingKind,
}

impl Var {
    /// Empty path means root scope.
    #[allow(dead_code)]
    pub fn path(&self) -> &[ScopeKind] {
        &self.path
    }

    pub fn kind(&self) -> BindingKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BindingKind {
    Var,
    Const,
    Let,
    Function,
    Param,
    Class,
    CatchClause,
    Import,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScopeKind {
    // Module,
    Arrow,
    Function,
    Block,
    Loop,
    Class,
    Switch,
    With,
    Catch,
}

struct Analyzer<'a> {
    scope: &'a mut Scope,
    path: &'a mut Vec<ScopeKind>,
}

impl Analyzer<'_> {
    fn declare_id(&mut self, kind: BindingKind, i: Id) {
        self.scope.vars.insert(
            i.clone(),
            Var {
                kind,
                path: self.path.clone(),
            },
        );
        self.scope.symbols.entry(i.0.clone()).or_default().push(i);
    }

    fn declare(&mut self, kind: BindingKind, i: &Ident) {
        self.declare_id(kind, i.to_id());
    }

    fn declare_pat(&mut self, kind: BindingKind, pat: &Pat) {
        let ids: Vec<Id> = find_ids(pat);

        for id in ids {
            self.declare_id(kind, id);
        }
    }

    fn visit_with_path<T>(&mut self, kind: ScopeKind, node: &T)
    where
        T: 'static + for<'any> VisitWith<Analyzer<'any>>,
    {
        self.path.push(kind);
        node.visit_with(node, self);
        self.path.pop();
    }

    fn with<F>(&mut self, kind: ScopeKind, op: F)
    where
        F: FnOnce(&mut Analyzer),
    {
        self.path.push(kind);
        op(self);
        self.path.pop();
    }
}

impl Visit for Analyzer<'_> {
    fn visit_arrow_expr(&mut self, n: &ArrowExpr, _: &dyn Node) {
        self.with(ScopeKind::Arrow, |a| n.visit_children_with(a))
    }

    /// Overriden not to add ScopeKind::Block
    fn visit_block_stmt_or_expr(&mut self, n: &BlockStmtOrExpr, _: &dyn Node) {
        match n {
            BlockStmtOrExpr::BlockStmt(s) => s.stmts.visit_with(n, self),
            BlockStmtOrExpr::Expr(e) => e.visit_with(n, self),
        }
    }

    fn visit_var_decl(&mut self, n: &VarDecl, _: &dyn Node) {
        n.decls.iter().for_each(|v| {
            v.init.visit_with(n, self);

            // If the class name and the variable name are the same like `let Foo = class Foo {}`,
            // this binding should be treated as `BindingKind::Class`.
            if let Some(expr) = &v.init {
                if let Expr::Class(ClassExpr {
                    ident: Some(class_name),
                    ..
                }) = &**expr
                {
                    if let Pat::Ident(var_name) = &v.name {
                        if var_name.id.sym == class_name.sym {
                            self.declare(BindingKind::Class, class_name);
                            return;
                        }
                    }
                }
            }

            self.declare_pat(
                match n.kind {
                    VarDeclKind::Var => BindingKind::Var,
                    VarDeclKind::Let => BindingKind::Let,
                    VarDeclKind::Const => BindingKind::Const,
                },
                &v.name,
            );
        });
    }

    /// Overriden not to add ScopeKind::Block
    fn visit_function(&mut self, n: &Function, _: &dyn Node) {
        n.decorators.visit_with(n, self);
        n.params.visit_with(n, self);

        // Don't add ScopeKind::Block
        match &n.body {
            Some(s) => s.stmts.visit_with(n, self),
            None => {}
        }
    }

    fn visit_fn_decl(&mut self, n: &FnDecl, _: &dyn Node) {
        self.declare(BindingKind::Function, &n.ident);

        self.visit_with_path(ScopeKind::Function, &n.function);
    }

    fn visit_fn_expr(&mut self, n: &FnExpr, _: &dyn Node) {
        if let Some(ident) = &n.ident {
            self.declare(BindingKind::Function, ident);
        }

        self.visit_with_path(ScopeKind::Function, &n.function);
    }

    fn visit_class_decl(&mut self, n: &ClassDecl, _: &dyn Node) {
        self.declare(BindingKind::Class, &n.ident);

        self.visit_with_path(ScopeKind::Class, &n.class);
    }

    fn visit_block_stmt(&mut self, n: &BlockStmt, _: &dyn Node) {
        self.visit_with_path(ScopeKind::Block, &n.stmts)
    }

    fn visit_catch_clause(&mut self, n: &CatchClause, _: &dyn Node) {
        if let Some(pat) = &n.param {
            self.declare_pat(BindingKind::CatchClause, pat);
        }
        self.visit_with_path(ScopeKind::Catch, &n.body)
    }

    fn visit_param(&mut self, n: &Param, _: &dyn Node) {
        self.declare_pat(BindingKind::Param, &n.pat);
    }

    fn visit_import_named_specifier(&mut self, n: &ImportNamedSpecifier, _: &dyn Node) {
        self.declare(BindingKind::Import, &n.local);
    }

    fn visit_import_default_specifier(&mut self, n: &ImportDefaultSpecifier, _: &dyn Node) {
        self.declare(BindingKind::Import, &n.local);
    }

    fn visit_import_star_as_specifier(&mut self, n: &ImportStarAsSpecifier, _: &dyn Node) {
        self.declare(BindingKind::Import, &n.local);
    }

    fn visit_with_stmt(&mut self, n: &WithStmt, _: &dyn Node) {
        n.obj.visit_with(n, self);
        self.with(ScopeKind::With, |a| n.body.visit_children_with(a))
    }

    fn visit_for_stmt(&mut self, n: &ForStmt, _: &dyn Node) {
        n.init.visit_with(n, self);
        n.update.visit_with(n, self);
        n.test.visit_with(n, self);

        self.visit_with_path(ScopeKind::Loop, &n.body);
    }

    fn visit_for_of_stmt(&mut self, n: &ForOfStmt, _: &dyn Node) {
        n.left.visit_with(n, self);
        n.right.visit_with(n, self);

        self.visit_with_path(ScopeKind::Loop, &n.body);
    }

    fn visit_for_in_stmt(&mut self, n: &ForInStmt, _: &dyn Node) {
        n.left.visit_with(n, self);
        n.right.visit_with(n, self);

        self.visit_with_path(ScopeKind::Loop, &n.body);
    }

    fn visit_do_while_stmt(&mut self, n: &DoWhileStmt, _: &dyn Node) {
        n.test.visit_with(n, self);

        self.visit_with_path(ScopeKind::Loop, &n.body);
    }

    fn visit_while_stmt(&mut self, n: &WhileStmt, _: &dyn Node) {
        n.test.visit_with(n, self);

        self.visit_with_path(ScopeKind::Loop, &n.body);
    }

    fn visit_switch_stmt(&mut self, n: &SwitchStmt, _: &dyn Node) {
        n.discriminant.visit_with(n, self);

        self.visit_with_path(ScopeKind::Switch, &n.cases);
    }
}

pub struct VisitFnDecl<'a> {
    cb: &'a mut dyn FnMut(&FnDecl),
}

impl<'a> Visit for VisitFnDecl<'a> {
    fn visit_fn_decl(&mut self, n: &FnDecl, _: &dyn Node) {
        (self.cb)(n);
    }
}

impl Compiler {
    pub fn intern_str(&mut self, s: &str) -> Symbol {
        s.intern()
    }
    pub fn intern(&mut self, id: &Ident) -> Symbol {
        let s: &str = &id.sym;
        s.intern()
    }
    pub fn get_ident(&mut self, id: &Ident) -> u32 {
        let s: &str = &id.sym;

        let interned = s.intern();
        self.builder.get_sym(interned)
    }
    pub fn compile_script(mut vm: &mut Runtime, p: &Script) -> GcPointer<CodeBlock> {
        let name = "<script>".intern();
        let mut code = CodeBlock::new(&mut vm, name, false);

        let mut compiler = Compiler {
            lci: Vec::new(),
            top_level: true,
            builder: ByteCodeBuilder {
                code: code,
                val_map: Default::default(),
                name_map: Default::default(),
            },
            fmap: Default::default(),
            vm: RuntimeRef(vm),
        };

        let is_strict = match p.body.get(0) {
            Some(ref body) => body.is_use_strict(),
            None => false,
        };
        code.top_level = true;
        code.strict = is_strict;
        compiler.compile(&p.body);
        // compiler.builder.emit(Opcode::OP_PUSH_UNDEFINED, &[], false);
        compiler.builder.emit(Opcode::OP_RET, &[], false);
        let result = compiler.builder.finish(&mut compiler.vm);

        result
    }
    pub fn compile_fn(&mut self, fun: &Function) {
        #[cfg(feature = "perf")]
        {
            self.vm.perf.set_prev_inst(crate::vm::perf::Perf::CODEGEN);
        }
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
        self.builder.code.strict = is_strict;
        match fun.body {
            Some(ref body) => {
                self.compile(&body.stmts);
            }
            None => {}
        }
        //self.builder.emit(Opcode::OP_PUSH_UNDEFINED, &[], false);
        self.builder.emit(Opcode::OP_RET, &[], false);
        self.builder.finish(&mut self.vm);
        #[cfg(feature = "perf")]
        {
            self.vm.perf.get_perf(crate::vm::perf::Perf::INVALID);
        }
    }
    pub fn compile(&mut self, body: &[Stmt]) {
        VisitFnDecl::visit(body, &mut |decl| {
            if true {
                let name = self.intern(&decl.ident);
                let mut rest = None;
                let mut params = vec![];
                for x in decl.function.params.iter() {
                    match x.pat {
                        Pat::Ident(ref x) => params.push(self.intern(&x.id)),
                        Pat::Rest(ref r) => match &*r.arg {
                            Pat::Ident(ref id) => {
                                rest = Some(self.intern(&id.id));
                            }
                            _ => unreachable!(),
                        },
                        _ => todo!(),
                    }
                }

                let mut code = CodeBlock::new(&mut self.vm, name, false);
                code.params = params;
                code.rest_param = rest;
                let mut compiler = Compiler {
                    lci: Vec::new(),
                    builder: ByteCodeBuilder {
                        code: code,

                        val_map: Default::default(),
                        name_map: Default::default(),
                    },
                    top_level: false,
                    fmap: Default::default(),
                    vm: RuntimeRef(&mut *self.vm),
                };

                compiler.compile_fn(&decl.function);
                let ix = self.builder.code.codes.len();
                self.builder.code.codes.push(code);
                self.fmap.insert(name, ix as _);
                let nix = self.builder.get_sym(name);
                self.builder
                    .emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
                if self.top_level {
                    self.builder.emit(Opcode::OP_SET_GLOBAL, &[nix as _], false);
                } else {
                    self.builder.emit(Opcode::OP_SET_VAR, &[nix as _], true);
                }
            }
        });
        for stmt in body.iter() {
            if contains_ident(stmt, "arguments") {
                self.builder.code.use_arguments = true;
                break;
            }
        }
        // self.builder.code.use_argumnets = contains_ident(body, "arguments");
        let scope = Scope::analyze_stmts(body);

        for var in scope.vars.iter() {
            match var.1.kind() {
                BindingKind::Var => {
                    let s: &str = &(var.0).0;
                    let name = self.intern_str(s);
                    if !self.builder.code.variables.contains(&name) {
                        self.builder.code.variables.push(name);
                    }
                }
                BindingKind::Function => {
                    let s: &str = &(var.0).0;
                    let name = self.intern_str(s);
                    if !self.builder.code.variables.contains(&name) {
                        self.builder.code.variables.push(name);
                    }
                }
                _ => (),
            }
        }

        for stmt in body {
            self.emit_stmt(stmt);
        }
    }
    pub fn emit(&mut self, expr: &Expr, used: bool) {
        match expr {
            Expr::Cond(cond) => {
                self.emit(&cond.test, true);
                let jelse = self.cjmp(false);
                self.emit(&cond.cons, used);

                let jend = self.jmp();
                jelse(self);
                self.emit(&cond.alt, used);
                jend(self);
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
                let mut code = CodeBlock::new(&mut self.vm, name, false);

                let mut compiler = Compiler {
                    lci: Vec::new(),
                    top_level: false,
                    builder: ByteCodeBuilder {
                        code: code,
                        val_map: Default::default(),
                        name_map: Default::default(),
                    },
                    fmap: Default::default(),
                    vm: RuntimeRef(&mut *self.vm),
                };
                code.strict = is_strict;
                for param in fun.params.iter() {
                    match param {
                        Pat::Ident(ref ident) => {
                            code.params.push(compiler.intern(&ident.id));
                        }
                        Pat::Rest(restpat) => match *restpat.arg {
                            Pat::Ident(ref id) => {
                                code.rest_param = Some(compiler.intern(&id.id));
                            }
                            _ => unreachable!(),
                        },
                        p => todo!("arrow param {:?}", p),
                    }
                }
                match &fun.body {
                    BlockStmtOrExpr::BlockStmt(block) => {
                        compiler.compile(&block.stmts);
                        compiler.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        compiler.builder.emit(Opcode::OP_RET, &[], false);
                    }
                    BlockStmtOrExpr::Expr(expr) => {
                        compiler.emit(expr, true);
                        compiler.builder.emit(Opcode::OP_RET, &[], false);
                    }
                }
                let code = compiler.builder.finish(&mut self.vm);
                let ix = self.builder.code.codes.len();
                self.builder.code.codes.push(code);
                let _nix = self.builder.get_sym(name);
                self.builder
                    .emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
            }
            Expr::Fn(fun) => {
                self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                let name = fun
                    .ident
                    .as_ref()
                    .map(|x| self.intern(x))
                    .unwrap_or_else(|| "<anonymous>".intern());
                let mut rest = None;
                let mut params = vec![];
                for x in fun.function.params.iter() {
                    match x.pat {
                        Pat::Ident(ref x) => params.push(self.intern(&x.id)),
                        Pat::Rest(ref r) => match &*r.arg {
                            Pat::Ident(ref id) => {
                                rest = Some(self.intern(&id.id));
                            }
                            _ => unreachable!(),
                        },
                        _ => todo!(),
                    }
                }

                let mut code = CodeBlock::new(&mut self.vm, name, false);
                code.params = params;

                code.rest_param = rest;
                let mut compiler = Compiler {
                    lci: Vec::new(),
                    top_level: false,
                    builder: ByteCodeBuilder {
                        code: code,
                        val_map: Default::default(),
                        name_map: Default::default(),
                    },
                    fmap: Default::default(),
                    vm: self.vm,
                };

                compiler.compile_fn(&fun.function);
                let ix = self.builder.code.codes.len();
                self.builder.code.codes.push(code);
                let nix = self.builder.get_sym(name);
                self.builder
                    .emit(Opcode::OP_GET_FUNCTION, &[ix as _], false);
                if name != "<anonymous>".intern() {
                    self.builder.emit(Opcode::OP_DUP, &[], false);
                    self.builder.emit(Opcode::OP_SET_VAR, &[nix as _], true);
                }
                self.builder.emit(Opcode::OP_POP_ENV, &[], false);
            }
            Expr::This(_) => {
                if used {
                    self.builder.emit(Opcode::OP_PUSH_THIS, &[], false);
                }
            }
            Expr::Array(array_lit) => {
                for expr in array_lit.elems.iter().rev() {
                    match expr {
                        Some(expr) => {
                            self.emit(&expr.expr, true);
                            if expr.spread.is_some() {
                                self.builder.emit(Opcode::OP_SPREAD, &[], false);
                            }
                        }
                        None => self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false),
                    }
                }
                self.builder
                    .emit(Opcode::OP_NEWARRAY, &[array_lit.elems.len() as u32], false);
                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false);
                }
            }

            Expr::Call(call) => {
                // self.builder.emit(Opcode::OP_PUSH_EMPTY, &[], false);
                let has_spread = call.args.iter().any(|x| x.spread.is_some());
                if has_spread {
                    for arg in call.args.iter().rev() {
                        self.emit(&arg.expr, true);
                        if arg.spread.is_some() {
                            self.builder.emit(Opcode::OP_SPREAD, &[], false);
                        }
                    }
                    self.builder
                        .emit(Opcode::OP_NEWARRAY, &[call.args.len() as u32], false);
                } else {
                    for arg in call.args.iter() {
                        self.emit(&arg.expr, true);
                        assert!(arg.spread.is_none());
                    }
                }

                match call.callee {
                    ExprOrSuper::Super(_) => todo!(), // todo super call
                    ExprOrSuper::Expr(ref expr) => match &**expr {
                        Expr::Member(member) => {
                            let name = if let Expr::Ident(id) = &*member.prop {
                                let s: &str = &id.sym;
                                let name = self.intern_str(s);
                                self.builder.get_sym(name)
                            } else {
                                unreachable!()
                            };
                            match member.obj {
                                ExprOrSuper::Expr(ref expr) => {
                                    self.emit(expr, true);
                                    self.builder.emit(Opcode::OP_DUP, &[], false);
                                }
                                ExprOrSuper::Super(_super) => {
                                    todo!()
                                }
                            }

                            self.builder.emit(Opcode::OP_GET_BY_ID, &[name], true);
                        }
                        _ => {
                            self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                            self.emit(&**expr, true);
                        }
                    },
                }
                if !has_spread {
                    self.builder
                        .emit(Opcode::OP_CALL, &[call.args.len() as u32], false);
                } else {
                    self.builder.emit(
                        Opcode::OP_CALL_BUILTIN,
                        &[call.args.len() as _, 0, 0],
                        false,
                    );
                }
                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::New(call) => {
                let argc = call.args.as_ref().map(|x| x.len() as u32).unwrap_or(0);
                let has_spread = if let Some(ref args) = call.args {
                    args.iter().any(|x| x.spread.is_some())
                } else {
                    false
                };
                if let Some(ref args) = call.args {
                    if has_spread {
                        for arg in args.iter().rev() {
                            self.emit(&arg.expr, true);
                            if arg.spread.is_some() {
                                self.builder.emit(Opcode::OP_SPREAD, &[], false);
                            }
                        }
                        self.builder.emit(Opcode::OP_NEWARRAY, &[argc], false);
                    } else {
                        for arg in args.iter() {
                            self.emit(&arg.expr, true);
                            assert!(arg.spread.is_none());
                        }
                    }
                }

                self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                self.emit(&*call.callee, true);
                if !has_spread {
                    self.builder.emit(Opcode::OP_NEW, &[argc], false);
                } else {
                    self.builder
                        .emit(Opcode::OP_CALL_BUILTIN, &[argc as _, 0, 1], false);
                }
                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Lit(literal) => {
                if used {
                    self.emit_lit(literal);
                }
            }

            Expr::Ident(name) => {
                let s: &str = &name.sym;
                let name = self.intern_str(s);
                if s == "undefined" {
                    self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false)
                } else if s == "NaN" {
                    let ix = self
                        .builder
                        .get_val(&mut self.vm, Val::Float(0x7ff8000000000000));
                    self.builder.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                } else if s == "Infinity" {
                    let ix = self
                        .builder
                        .get_val(&mut self.vm, Val::Float(std::f64::INFINITY.to_bits()));
                    self.builder.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                } else {
                    assert!(self.vm.description(name) != "undefined");
                    let ix = self.builder.get_sym(name);
                    if used {
                        self.builder.emit(Opcode::OP_GET_VAR, &[ix], true);
                    }
                }
            }

            Expr::Member(member) => {
                let name = if !member.computed {
                    if let Expr::Ident(id) = &*member.prop {
                        let s: &str = &id.sym;
                        let name = s.intern();
                        Some(self.builder.get_sym(name))
                    } else {
                        self.emit(&member.prop, true);
                        None
                    }
                } else {
                    self.emit(&member.prop, true);
                    None
                };
                match member.obj {
                    ExprOrSuper::Expr(ref expr) => {
                        self.emit(expr, true);
                    }
                    ExprOrSuper::Super(_super) => {
                        todo!()
                    }
                }

                if let Some(ix) = name {
                    self.builder.emit(Opcode::OP_GET_BY_ID, &[ix], true);
                } else {
                    self.builder.emit(Opcode::OP_GET_BY_VAL, &[], false);
                }
                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Unary(unary) => {
                self.emit(&unary.arg, true);
                match unary.op {
                    UnaryOp::Minus => self.builder.emit(Opcode::OP_NEG, &[], false),
                    UnaryOp::Plus => self.builder.emit(Opcode::OP_POS, &[], false),
                    UnaryOp::Tilde => self.builder.emit(Opcode::OP_NOT, &[], false),
                    UnaryOp::Bang => self.builder.emit(Opcode::OP_LOGICAL_NOT, &[], false),
                    UnaryOp::TypeOf => self.builder.emit(Opcode::OP_TYPEOF, &[], false),

                    _ => todo!("{:?}", unary.op),
                }
                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false)
                }
            }

            Expr::Object(object_lit) => {
                self.builder.emit(Opcode::OP_NEWOBJECT, &[], false);
                for prop in object_lit.props.iter() {
                    match prop {
                        PropOrSpread::Prop(prop) => match &**prop {
                            Prop::Shorthand(ident) => {
                                self.builder.emit(Opcode::OP_DUP, &[], false);
                                let ix = self.intern(ident);
                                let sym = self.builder.get_sym(ix);
                                self.builder.emit(Opcode::OP_GET_VAR, &[sym], true);
                                self.builder.emit(Opcode::OP_SWAP, &[], false);
                                self.builder.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                            }
                            Prop::KeyValue(assign) => {
                                self.builder.emit(Opcode::OP_DUP, &[], false);
                                self.emit(&assign.value, true);
                                match assign.key {
                                    PropName::Ident(ref id) => {
                                        let ix = self.intern(id);
                                        let sym = self.builder.get_sym(ix);
                                        self.builder.emit(Opcode::OP_SWAP, &[], false);
                                        self.builder.emit(Opcode::OP_PUT_BY_ID, &[sym], true);
                                    }
                                    PropName::Str(ref s) => {
                                        let ix = self
                                            .builder
                                            .get_val(&mut self.vm, Val::Str(s.value.to_string()));
                                        self.builder.emit(Opcode::OP_SWAP, &[], false);
                                        self.builder.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                                        self.builder.emit(Opcode::OP_SWAP, &[], false);
                                        self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                                    }
                                    PropName::Num(n) => {
                                        let val = n.value;
                                        if val as i32 as f64 == val {
                                            self.builder.emit(Opcode::OP_SWAP, &[], false);
                                            self.builder.emit(
                                                Opcode::OP_PUSH_INT,
                                                &[val as i32 as u32],
                                                false,
                                            );
                                            self.builder.emit(Opcode::OP_SWAP, &[], false);
                                            self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                                        } else {
                                            let ix = self
                                                .builder
                                                .get_val(&mut self.vm, Val::Float(val.to_bits()));
                                            self.builder.emit(Opcode::OP_SWAP, &[], false);
                                            self.builder.emit(
                                                Opcode::OP_PUSH_LITERAL,
                                                &[ix],
                                                false,
                                            );
                                            self.builder.emit(Opcode::OP_SWAP, &[], false);
                                            self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            p => todo!("{:?}", p),
                        },
                        _ => todo!(),
                    }
                }
            }
            Expr::Paren(p) => {
                self.emit(&p.expr, used);
            }
            Expr::Assign(assign) => match assign.op {
                AssignOp::Assign => match &assign.left {
                    PatOrExpr::Pat(x) => {
                        self.emit(&assign.right, true);
                        if used {
                            self.builder.emit(Opcode::OP_DUP, &[], false);
                        }
                        self.generate_pat_store(&**x, false, false);
                    }
                    PatOrExpr::Expr(e) => match &**e {
                        Expr::Member(member) => {
                            self.emit(&assign.right, true);
                            if used {
                                self.builder.emit(Opcode::OP_DUP, &[], false);
                            }
                            let name = if !member.computed {
                                if let Expr::Ident(id) = &*member.prop {
                                    let s: &str = &id.sym;
                                    let name = s.intern();
                                    Some(self.builder.get_sym(name))
                                } else {
                                    self.emit(&member.prop, true);
                                    None
                                }
                            } else {
                                self.emit(&member.prop, true);
                                None
                            };
                            match member.obj {
                                ExprOrSuper::Expr(ref expr) => {
                                    self.emit(expr, true);
                                }
                                ExprOrSuper::Super(_super) => {
                                    todo!()
                                }
                            }

                            if let Some(ix) = name {
                                self.builder.emit(Opcode::OP_PUT_BY_ID, &[ix], true);
                            } else {
                                self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                            }
                        }
                        Expr::Ident(id) => {
                            self.emit(&assign.right, true);
                            let sym = self.get_ident(&id);
                            self.builder.emit(Opcode::OP_SET_VAR, &[sym], true);
                        }
                        e => todo!("{:?}", e,),
                    },
                },
                op => {
                    self.emit_load_from(&assign.left);
                    if used {
                        self.builder.emit(Opcode::OP_DUP, &[], false);
                    }
                    self.emit(&assign.right, true);
                    self.builder.emit(Opcode::OP_SWAP, &[], false);
                    let op = match op {
                        AssignOp::AddAssign => Opcode::OP_ADD,
                        AssignOp::SubAssign => Opcode::OP_SUB,
                        AssignOp::MulAssign => Opcode::OP_MUL,
                        AssignOp::DivAssign => Opcode::OP_DIV,
                        AssignOp::BitAndAssign => Opcode::OP_AND,
                        AssignOp::BitOrAssign => Opcode::OP_OR,
                        AssignOp::BitXorAssign => Opcode::OP_XOR,
                        AssignOp::ModAssign => Opcode::OP_REM,

                        _ => todo!(),
                    };
                    self.builder.emit(op, &[], false);
                    self.emit_store_from(&assign.left);
                }
            },
            Expr::Bin(binary) => {
                match binary.op {
                    BinaryOp::LogicalOr => {
                        self.emit(&binary.left, true);
                        self.builder.emit(Opcode::OP_DUP, &[], false);
                        let jtrue = self.cjmp(true);
                        self.builder.emit(Opcode::OP_POP, &[], false);
                        self.emit(&binary.right, true);
                        //let end = self.jmp();
                        jtrue(self);
                        // self.builder.emit(Opcode::OP_PUSH_TRUE, &[], false);
                        //end(self);
                        if !used {
                            self.builder.emit(Opcode::OP_POP, &[], false);
                        }
                        return;
                    }
                    BinaryOp::LogicalAnd => {
                        self.emit(&binary.left, true);
                        self.builder.emit(Opcode::OP_DUP, &[], false);
                        let jfalse = self.cjmp(false);
                        self.builder.emit(Opcode::OP_POP, &[], false);
                        self.emit(&binary.right, true);
                        let end = self.jmp();
                        jfalse(self);
                        end(self);
                        if !used {
                            self.builder.emit(Opcode::OP_POP, &[], false);
                        }
                        return;
                    }

                    _ => (),
                }
                self.emit(&binary.right, true);
                self.emit(&binary.left, true);

                match binary.op {
                    BinaryOp::Add => {
                        self.builder.emit(Opcode::OP_ADD, &[], false);
                    }
                    BinaryOp::Sub => {
                        self.builder.emit(Opcode::OP_SUB, &[], false);
                    }
                    BinaryOp::Mul => {
                        self.builder.emit(Opcode::OP_MUL, &[], false);
                    }
                    BinaryOp::Div => {
                        self.builder.emit(Opcode::OP_DIV, &[], false);
                    }
                    BinaryOp::EqEq => {
                        self.builder.emit(Opcode::OP_EQ, &[], false);
                    }
                    BinaryOp::EqEqEq => self.builder.emit(Opcode::OP_STRICTEQ, &[], false),
                    BinaryOp::NotEq => self.builder.emit(Opcode::OP_NEQ, &[], false),
                    BinaryOp::NotEqEq => self.builder.emit(Opcode::OP_NSTRICTEQ, &[], false),
                    BinaryOp::Gt => self.builder.emit(Opcode::OP_GREATER, &[], false),
                    BinaryOp::GtEq => self.builder.emit(Opcode::OP_GREATEREQ, &[], false),
                    BinaryOp::Lt => self.builder.emit(Opcode::OP_LESS, &[], false),
                    BinaryOp::LtEq => self.builder.emit(Opcode::OP_LESSEQ, &[], false),
                    BinaryOp::In => self.builder.emit(Opcode::OP_IN, &[], false),
                    BinaryOp::Mod => self.builder.emit(Opcode::OP_REM, &[], false),
                    _ => todo!(),
                }

                if !used {
                    self.builder.emit(Opcode::OP_POP, &[], false);
                }
            }
            Expr::Update(update) => {
                let op = match update.op {
                    UpdateOp::PlusPlus => Opcode::OP_ADD,
                    UpdateOp::MinusMinus => Opcode::OP_SUB,
                };
                if update.prefix {
                    self.builder
                        .emit(Opcode::OP_PUSH_INT, &[1i32 as u32], false);
                    self.emit(&update.arg, true);
                    self.builder.emit(op, &[], false);
                    if used {
                        self.builder.emit(Opcode::OP_DUP, &[], false);
                    }
                    self.emit_store_expr(&update.arg);
                } else {
                    self.builder
                        .emit(Opcode::OP_PUSH_INT, &[1i32 as u32], false);
                    self.emit(&update.arg, true);
                    if used {
                        self.builder.emit(Opcode::OP_DUP, &[], false);
                    }
                    self.builder.emit(op, &[], false);

                    self.emit_store_expr(&update.arg);
                }
            }
            _ => todo!("{:?}", expr),
        }
    }

    pub fn emit_load_from(&mut self, p: &PatOrExpr) {
        match p {
            PatOrExpr::Pat(p) => match &**p {
                Pat::Ident(id) => {
                    let ix = self.get_ident(&id.id);
                    self.builder.emit(Opcode::OP_GET_VAR, &[ix], true);
                }
                _ => todo!("{:?}", p),
            },
            PatOrExpr::Expr(expr) => self.emit_load_expr(&**expr),
        }
    }
    pub fn emit_load_expr(&mut self, e: &Expr) {
        match e {
            Expr::Ident(id) => {
                let ix = self.get_ident(id);
                self.builder.emit(Opcode::OP_GET_VAR, &[ix], true);
            }
            Expr::Member(member) => {
                let name = if !member.computed {
                    if let Expr::Ident(id) = &*member.prop {
                        let s: &str = &id.sym;
                        let name = s.intern();
                        Some(self.builder.get_sym(name))
                    } else {
                        self.emit(&member.prop, true);
                        None
                    }
                } else {
                    self.emit(&member.prop, true);
                    None
                };
                match member.obj {
                    ExprOrSuper::Expr(ref expr) => {
                        self.emit(expr, true);
                    }
                    ExprOrSuper::Super(_super) => {
                        todo!()
                    }
                }

                if let Some(ix) = name {
                    self.builder.emit(Opcode::OP_GET_BY_ID, &[ix], true);
                } else {
                    self.builder.emit(Opcode::OP_GET_BY_VAL, &[], false);
                }
            }
            e => todo!("{:?}", e,),
        }
    }
    pub fn emit_store_from(&mut self, p: &PatOrExpr) {
        match p {
            PatOrExpr::Pat(p) => match &**p {
                Pat::Ident(id) => {
                    let ix = self.get_ident(&id.id);
                    self.builder.emit(Opcode::OP_SET_VAR, &[ix], true);
                }
                _ => todo!("{:?}", p),
            },
            PatOrExpr::Expr(expr) => self.emit_store_expr(&**expr),
        }
    }
    pub fn emit_store_expr(&mut self, e: &Expr) {
        match e {
            Expr::Ident(id) => {
                let ix = self.get_ident(id);
                self.builder.emit(Opcode::OP_SET_VAR, &[ix], true);
            }
            Expr::Member(member) => {
                let name = if !member.computed {
                    if let Expr::Ident(id) = &*member.prop {
                        let s: &str = &id.sym;
                        let name = s.intern();
                        Some(self.builder.get_sym(name))
                    } else {
                        self.emit(&member.prop, true);
                        None
                    }
                } else {
                    self.emit(&member.prop, true);
                    None
                };
                match member.obj {
                    ExprOrSuper::Expr(ref expr) => {
                        self.emit(expr, true);
                    }
                    ExprOrSuper::Super(_super) => {
                        todo!()
                    }
                }

                if let Some(ix) = name {
                    self.builder.emit(Opcode::OP_PUT_BY_ID, &[ix], true);
                } else {
                    self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                }
            }
            e => todo!("{:?}", e,),
        }
    }
    pub fn push_lci(&mut self, _continue_target: u32) {
        self.lci.push(LoopControlInfo {
            continues: vec![],
            breaks: vec![],
            scope_depth: 0,
        });
    }

    pub fn pop_lci(&mut self) {
        let mut lci = self.lci.pop().unwrap();
        while let Some(break_) = lci.breaks.pop() {
            break_(self);
        }
    }
    pub fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => {
                self.emit(&expr.expr, false);
            }
            Stmt::Block(block) => {
                self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth += 1;
                }
                for stmt in block.stmts.iter() {
                    self.emit_stmt(stmt);
                }
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth -= 1;
                }
                self.builder.emit(Opcode::OP_POP_ENV, &[], false);
            }
            Stmt::Return(ret) => {
                match ret.arg {
                    Some(ref arg) => self.emit(&**arg, true),
                    None => self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false),
                }
                // self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                self.builder.emit(Opcode::OP_RET, &[], false);
            }
            Stmt::Break(_) => {
                for _ in 0..self.lci.last().map(|x| x.scope_depth - 1).unwrap() {
                    self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                }
                let br = self.jmp();
                self.lci.last_mut().unwrap().breaks.push(Box::new(br));
            }
            Stmt::Continue(_) => {
                for _ in 0..self.lci.last().map(|x| x.scope_depth).unwrap() {
                    self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                }
                // self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                let j = self.jmp();
                self.lci.last_mut().unwrap().continues.push(Box::new(j));
            }
            Stmt::For(for_stmt) => {
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth += 1;
                }
                self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                match for_stmt.init {
                    Some(ref init) => match init {
                        VarDeclOrExpr::Expr(ref e) => {
                            self.emit(e, false);
                        }
                        VarDeclOrExpr::VarDecl(ref decl) => {
                            self.emit_var_decl(decl);
                        }
                    },
                    None => {}
                }

                let head = self.builder.code.code.len();
                self.push_lci(head as _);
                match for_stmt.test {
                    Some(ref test) => {
                        self.emit(&**test, true);
                    }
                    None => {
                        self.builder.emit(Opcode::OP_PUSH_TRUE, &[], false);
                    }
                }
                let jend = self.cjmp(false);
                self.emit_stmt(&for_stmt.body);
                let skip = self.jmp();
                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth -= 1;
                }
                //self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                skip(self);
                if let Some(fin) = &for_stmt.update {
                    self.emit(&**fin, false);
                }
                self.goto(head as _);
                self.pop_lci();
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth -= 1;
                }
                self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                jend(self);
                if let Some(lci) = self.lci.last_mut() {
                    lci.scope_depth -= 1;
                }
                self.builder.emit(Opcode::OP_POP_ENV, &[], false);
            }
            Stmt::While(while_stmt) => {
                let head = self.builder.code.code.len();
                self.push_lci(head as _);
                self.emit(&while_stmt.test, true);
                let jend = self.cjmp(false);
                self.emit_stmt(&while_stmt.body);

                while let Some(c) = self.lci.last_mut().unwrap().continues.pop() {
                    c(self);
                }
                self.goto(head);
                jend(self);
                self.pop_lci();
            }
            Stmt::If(if_stmt) => {
                self.emit(&if_stmt.test, true);
                let jelse = self.cjmp(false);
                self.emit_stmt(&if_stmt.cons);
                match if_stmt.alt {
                    None => {
                        jelse(self);
                    }
                    Some(ref alt) => {
                        let jend = self.jmp();
                        jelse(self);
                        self.emit_stmt(&**alt);
                        jend(self);
                    }
                }
            }
            Stmt::Decl(decl) => match decl {
                Decl::Var(var) => {
                    self.emit_var_decl(var);
                }
                Decl::Fn(fun) => {
                    let s: &str = &fun.ident.sym;
                    let sym = s.intern();
                    let ix = *self.fmap.get(&sym).unwrap();
                    self.builder.emit(Opcode::OP_GET_FUNCTION, &[ix], false);
                    let nix = self.builder.get_sym(sym);
                    self.builder.emit(Opcode::OP_SET_VAR, &[nix], true);
                }
                _ => (),
            },

            Stmt::Empty(_) => {}
            Stmt::Throw(throw) => {
                self.emit(&throw.arg, true);
                self.builder.emit(Opcode::OP_THROW, &[], false);
            }
            Stmt::Try(try_stmt) => {
                let try_push = self.try_();
                if !try_stmt.block.stmts.is_empty() {
                    if let Some(lci) = self.lci.last_mut() {
                        lci.scope_depth += 1;
                    }
                    self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                }
                for stmt in try_stmt.block.stmts.iter() {
                    self.emit_stmt(stmt);
                }
                if !try_stmt.block.stmts.is_empty() {
                    self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                }
                let jfinally = self.jmp();
                try_push(self);
                let jcatch_finally = match try_stmt.handler {
                    Some(ref catch) => {
                        if !catch.body.stmts.is_empty() {
                            if let Some(lci) = self.lci.last_mut() {
                                lci.scope_depth += 1;
                            }
                            self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                        }
                        match catch.param {
                            Some(ref pat) => {
                                self.generate_pat_store(pat, true, true);
                            }
                            None => {
                                self.builder.emit(Opcode::OP_POP, &[], false);
                            }
                        }
                        for stmt in catch.body.stmts.iter() {
                            self.emit_stmt(stmt);
                        }
                        if !catch.body.stmts.is_empty() {
                            if let Some(lci) = self.lci.last_mut() {
                                lci.scope_depth -= 1;
                            }
                            self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                        }
                        self.jmp()
                    }
                    None => {
                        self.builder.emit(Opcode::OP_POP, &[], false);
                        self.jmp()
                    }
                };

                jfinally(self);
                jcatch_finally(self);
                match try_stmt.finalizer {
                    Some(ref block) => {
                        if !block.stmts.is_empty() {
                            if let Some(lci) = self.lci.last_mut() {
                                lci.scope_depth += 1;
                            }
                            self.builder.emit(Opcode::OP_PUSH_ENV, &[], true);
                        }
                        for stmt in block.stmts.iter() {
                            self.emit_stmt(stmt);
                        }
                        if !block.stmts.is_empty() {
                            if let Some(lci) = self.lci.last_mut() {
                                lci.scope_depth -= 1;
                            }
                            self.builder.emit(Opcode::OP_POP_ENV, &[], false);
                        }
                    }
                    None => {}
                }
            }

            x => todo!("{:?}", x),
        }
    }
    pub fn generate_pat_store(&mut self, pat: &Pat, decl: bool, mutable: bool) {
        match pat {
            Pat::Ident(id) => {
                let name = self.get_ident(&id.id);
                if decl && mutable {
                    self.builder.emit(Opcode::OP_DECL_LET, &[name], true);
                } else if decl && !mutable {
                    self.builder.emit(Opcode::OP_DECL_CONST, &[name], true);
                }

                if !decl {
                    self.builder.emit(Opcode::OP_SET_VAR, &[name], true);
                }
            }
            Pat::Expr(e) => match &**e {
                Expr::Member(member) => {
                    let name = if !member.computed {
                        if let Expr::Ident(id) = &*member.prop {
                            let s: &str = &id.sym;
                            let name = s.intern();
                            Some(self.builder.get_sym(name))
                        } else {
                            self.emit(&member.prop, true);
                            None
                        }
                    } else {
                        self.emit(&member.prop, true);
                        None
                    };
                    match member.obj {
                        ExprOrSuper::Expr(ref expr) => {
                            self.emit(expr, true);
                        }
                        ExprOrSuper::Super(_super) => {
                            todo!()
                        }
                    }

                    if let Some(ix) = name {
                        self.builder.emit(Opcode::OP_PUT_BY_ID, &[ix], true);
                    } else {
                        self.builder.emit(Opcode::OP_PUT_BY_VAL, &[], false);
                    }
                }
                _ => todo!(),
            },
            _ => todo!(),
        }
    }
    pub fn try_(&mut self) -> impl FnOnce(&mut Self) {
        let p = self.builder.code.code.len();
        self.builder.emit(Opcode::OP_PUSH_CATCH, &[0], false);

        move |this: &mut Self| {
            let to = this.builder.code.code.len() - (p + 5);
            let ins = Opcode::OP_PUSH_CATCH;
            let bytes = (to as u32).to_le_bytes();
            this.builder.code.code[p] = ins as u8;
            this.builder.code.code[p + 1] = bytes[0];
            this.builder.code.code[p + 2] = bytes[1];
            this.builder.code.code[p + 3] = bytes[2];
            this.builder.code.code[p + 4] = bytes[3];
        }
    }
    pub fn cjmp(&mut self, cond: bool) -> impl FnOnce(&mut Self) {
        let p = self.builder.code.code.len();
        self.builder.emit(Opcode::OP_JMP, &[0], false);

        move |this: &mut Self| {
            //  this.builder.emit(Opcode::OP_NOP, &[], false);
            let to = this.builder.code.code.len() - (p + 5);
            let ins = if cond {
                Opcode::OP_JMP_IF_TRUE
            } else {
                Opcode::OP_JMP_IF_FALSE
            };
            let bytes = (to as u32).to_le_bytes();
            this.builder.code.code[p] = ins as u8;
            this.builder.code.code[p + 1] = bytes[0];
            this.builder.code.code[p + 2] = bytes[1];
            this.builder.code.code[p + 3] = bytes[2];
            this.builder.code.code[p + 4] = bytes[3];
        }
    }
    pub fn goto(&mut self, to: usize) {
        let at = self.builder.code.code.len() as i32 + 5;
        self.builder
            .emit(Opcode::OP_JMP, &[(to as i32 - at) as u32], false);
    }
    pub fn jmp(&mut self) -> impl FnOnce(&mut Self) {
        let p = self.builder.code.code.len();
        self.builder.emit(Opcode::OP_JMP, &[0], false);

        move |this: &mut Self| {
            // this.builder.emit(Opcode::OP_NOP, &[], false);
            let to = this.builder.code.code.len() - (p + 5);
            let bytes = (to as u32).to_le_bytes();
            this.builder.code.code[p] = Opcode::OP_JMP as u8;
            this.builder.code.code[p + 1] = bytes[0];
            this.builder.code.code[p + 2] = bytes[1];
            this.builder.code.code[p + 3] = bytes[2];
            this.builder.code.code[p + 4] = bytes[3];
            //this.builder.code.code[p] = ins as u8;
        }
    }
    pub fn emit_lit(&mut self, lit: &Lit) {
        match lit {
            Lit::Null(_) => self.builder.emit(Opcode::OP_PUSH_NULL, &[], false),
            Lit::Num(x) => {
                let val = x.value;
                if val as i32 as f64 == val {
                    self.builder
                        .emit(Opcode::OP_PUSH_INT, &[val as i32 as u32], false);
                } else {
                    let ix = self
                        .builder
                        .get_val(&mut self.vm, Val::Float(val.to_bits()));
                    self.builder.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
                }
            }
            Lit::Str(x) => {
                let val = x.value.to_string();
                let mut vm = self.vm;
                let ix = self.builder.get_val(&mut vm, Val::Str(val));
                self.builder.emit(Opcode::OP_PUSH_LITERAL, &[ix], false);
            }
            Lit::Bool(x) => {
                if x.value {
                    self.builder.emit(Opcode::OP_PUSH_TRUE, &[], false);
                } else {
                    self.builder.emit(Opcode::OP_PUSH_FALSE, &[], false);
                }
            }
            _ => todo!("Other literals"),
        }
    }
    pub fn emit_var_decl(&mut self, var: &VarDecl) {
        for decl in var.decls.iter() {
            match &decl.name {
                Pat::Ident(name) => match decl.init {
                    Some(ref init) => {
                        let s: &str = &name.id.sym;
                        let name = s.intern();
                        let ix = self.builder.get_sym(name);
                        self.emit(init, true);
                        match var.kind {
                            VarDeclKind::Let => self.builder.emit(Opcode::OP_DECL_LET, &[ix], true),
                            VarDeclKind::Const => {
                                self.builder.emit(Opcode::OP_DECL_CONST, &[ix], true)
                            }
                            VarDeclKind::Var => {
                                if self.top_level {
                                    self.builder.emit(Opcode::OP_SET_GLOBAL, &[ix], false);
                                } else {
                                    self.builder.emit(Opcode::OP_SET_VAR, &[ix], true);
                                }
                            }
                        }
                    }
                    None => {
                        let s: &str = &name.id.sym;
                        let name = s.intern();
                        let ix = self.builder.get_sym(name);
                        self.builder.emit(Opcode::OP_PUSH_UNDEF, &[], false);
                        match var.kind {
                            VarDeclKind::Let => {
                                self.builder.emit(Opcode::OP_DECL_LET, &[ix], true);
                                return;
                            }
                            VarDeclKind::Const => {
                                self.builder.emit(Opcode::OP_DECL_CONST, &[ix], true);
                                return;
                            }
                            VarDeclKind::Var => {}
                        }
                        self.builder.emit(Opcode::OP_SET_VAR, &[ix], true);
                    }
                },
                _ => todo!(),
            }
        }
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
