use hashbrown::HashMap;
use scope_analyzer::{Scope, VisitFnDecl};
use swc_ecmascript::ast::*;

use crate::{
    bytecode::opcodes::*,
    bytecode::*,
    heap::cell::{Gc, Trace, Tracer},
    runtime::symbol::Symbol,
    vm::VirtualMachineRef,
};

pub mod ast_parser;
pub mod scope_analyzer;

pub struct Compiler {
    builder: ByteCodeBuilder,
    vm: VirtualMachineRef,
    fmap: HashMap<Symbol, u32>,
}
impl Compiler {
    pub fn intern_str(&mut self, s: &str) -> Symbol {
        match s {
            "length" => Symbol::length(),
            "prototype" => Symbol::prototype(),
            "arguments" => Symbol::arguments(),
            "caller" => Symbol::caller(),
            "callee" => Symbol::callee(),
            "toString" => Symbol::toString(),
            _ => {
                let interned = self.vm.intern(s);
                interned
            }
        }
    }
    pub fn intern(&mut self, id: &Ident) -> Symbol {
        let s: &str = &id.sym;
        match s {
            "length" => Symbol::length(),
            "prototype" => Symbol::prototype(),
            "arguments" => Symbol::arguments(),
            "caller" => Symbol::caller(),
            "callee" => Symbol::callee(),
            "toString" => Symbol::toString(),
            _ => {
                let interned = self.vm.intern(s);
                interned
            }
        }
    }
    pub fn get_ident(&mut self, id: &Ident) -> u32 {
        let s: &str = &id.sym;
        match s {
            "length" => self.builder.get_sym(Symbol::length()),
            "prototype" => self.builder.get_sym(Symbol::prototype()),
            "arguments" => self.builder.get_sym(Symbol::arguments()),
            "caller" => self.builder.get_sym(Symbol::caller()),
            "callee" => self.builder.get_sym(Symbol::callee()),
            "toString" => self.builder.get_sym(Symbol::toString()),
            _ => {
                let interned = self.vm.intern(s);
                self.builder.get_sym(interned)
            }
        }
    }

    pub fn compile_script(mut vm: VirtualMachineRef, p: &Script) -> Gc<ByteCode> {
        let ctx = vm.space().new_local_context();
        let name = vm.intern("<global>");
        let mut code = ctx.new_local(ByteCode::new(&mut vm, name, &[], false));
        let mut compiler = Compiler {
            builder: ByteCodeBuilder {
                code: *code,
                val_map: Default::default(),
                name_map: Default::default(),
            },
            fmap: Default::default(),
            vm: vm,
        };

        let is_strict = match p.body.get(0) {
            Some(ref body) => match body {
                Stmt::Expr(ref e) => match &*e.expr {
                    Expr::Ident(x) => {
                        let s: &str = &x.sym;
                        s == "use strict"
                    }
                    _ => false,
                },
                _ => false,
            },
            None => false,
        };
        code.strict = is_strict;
        compiler.compile(&p.body);
        compiler.builder.emit(Op::OP_PUSH_UNDEFINED, &[], false);
        compiler.builder.emit(Op::OP_RET, &[], false);
        compiler.builder.finish()
    }
    pub fn compile_fn(&mut self, fun: &Function) {
        let is_strict = match fun.body {
            Some(ref body) => {
                if body.stmts.is_empty() {
                    false
                } else {
                    match body.stmts[0] {
                        Stmt::Expr(ref e) => match &*e.expr {
                            Expr::Ident(x) => {
                                let s: &str = &x.sym;
                                s == "use strict"
                            }
                            _ => false,
                        },
                        _ => false,
                    }
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
        self.builder.emit(Op::OP_PUSH_UNDEFINED, &[], false);
        self.builder.emit(Op::OP_RET, &[], false);
        self.builder.finish();
    }
    pub fn compile(&mut self, body: &[Stmt]) {
        let ctx = self.vm.space().new_local_context();
        let mut i = 0;
        VisitFnDecl::visit(body, &mut |decl| {
            let name = self.intern(&decl.ident);
            let params = decl
                .function
                .params
                .iter()
                .map(|x: &Param| match x.pat {
                    Pat::Ident(ref x) => self.intern(x),
                    _ => todo!(),
                })
                .collect::<Vec<Symbol>>();
            let mut code = ctx.new_local(ByteCode::new(&mut self.vm, name, &params, false));
            let mut compiler = Compiler {
                builder: ByteCodeBuilder {
                    code: *code,
                    val_map: Default::default(),
                    name_map: Default::default(),
                },
                fmap: Default::default(),
                vm: self.vm,
            };

            compiler.compile_fn(&decl.function);
            let ix = self.builder.code.codes.len();
            self.builder.code.codes.push(*code);
            self.fmap.insert(name, ix as _);
            let nix = self.builder.get_sym(name);
            self.builder.emit(Op::OP_GET_FUNCTION, &[ix as _], false);

            self.builder.emit(Op::OP_SET_VAR, &[nix as _], true);
        });
        let mut scope = Scope::analyze_stmts(body);

        for var in scope.vars.iter() {
            match var.1.kind() {
                scope_analyzer::BindingKind::Var => {
                    let s: &str = &(var.0).0;
                    let name = self.intern_str(s);
                    if !self.builder.code.var_names.contains(&name) {
                        self.builder.code.var_names.push(name);
                    }
                }
                scope_analyzer::BindingKind::Function => {
                    let s: &str = &(var.0).0;
                    let name = self.intern_str(s);
                    if !self.builder.code.var_names.contains(&name) {
                        self.builder.code.var_names.push(name);
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
            Expr::Call(call) => {
                for arg in call.args.iter() {
                    if arg.spread.is_some() {
                        todo!("spread");
                    }
                    self.emit(&arg.expr, true);
                }

                match call.callee {
                    ExprOrSuper::Super(_) => todo!(), // todo super call
                    ExprOrSuper::Expr(ref expr) => match &**expr {
                        Expr::Member(member) => {
                            let name = if let Expr::Ident(id) = &*member.prop {
                                let s: &str = &id.sym;
                                let name = self.vm.intern(s);
                                self.builder.get_sym(name)
                            } else {
                                unreachable!()
                            };
                            match member.obj {
                                ExprOrSuper::Expr(ref expr) => {
                                    self.emit(expr, true);
                                    self.builder.emit(Op::OP_DUP, &[], false);
                                }
                                ExprOrSuper::Super(_super) => {
                                    todo!()
                                }
                            }

                            self.builder.emit(Op::OP_GET_PROP, &[name], true);
                        }
                        _ => {
                            self.builder.emit(Op::OP_PUSH_EMPTY, &[], false);
                            self.emit(&**expr, true);
                        }
                    },
                }

                self.builder
                    .emit(Op::OP_CALL, &[call.args.len() as u32], false);
            }
            Expr::New(call) => {
                let argc = call.args.as_ref().map(|x| x.len() as u32).unwrap_or(0);
                if let Some(ref args) = call.args {
                    for arg in args.iter() {
                        if arg.spread.is_some() {
                            todo!("spread");
                        }
                        self.emit(&arg.expr, true);
                    }
                }

                self.builder.emit(Op::OP_PUSH_EMPTY, &[], false);
                self.emit(&*call.callee, true);

                self.builder.emit(Op::OP_NEW, &[argc], false);
            }
            Expr::Lit(literal) => {
                if used {
                    self.emit_lit(literal);
                }
            }

            Expr::Ident(name) => {
                let s: &str = &name.sym;
                let name = self.vm.intern(s);
                let ix = self.builder.get_sym(name);
                if used {
                    self.builder.emit(Op::OP_GET_VAR, &[ix], true);
                }
            }

            Expr::Member(member) => {
                let name = if let Expr::Ident(id) = &*member.prop {
                    let s: &str = &id.sym;
                    let name = self.vm.intern(s);
                    Some(self.builder.get_sym(name))
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
                    self.builder.emit(Op::OP_GET_PROP, &[ix], true);
                } else {
                    self.builder.emit(Op::OP_GET, &[], false);
                }
                if !used {
                    self.builder.emit(Op::OP_DROP, &[], false);
                }
            }
            Expr::Assign(assign) => match &assign.left {
                PatOrExpr::Pat(x) => match &**x {
                    Pat::Ident(id) => {
                        self.emit(&assign.right, true);
                        let ix = self.get_ident(id);
                        self.builder.emit(Op::OP_SET_VAR, &[ix], true);
                    }
                    Pat::Expr(e) => match &**e {
                        Expr::Member(member) => {
                            self.emit(&assign.right, true);
                            let name = if let Expr::Ident(id) = &*member.prop {
                                let s: &str = &id.sym;
                                let name = self.vm.intern(s);
                                Some(self.builder.get_sym(name))
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
                                self.builder.emit(Op::OP_SET_PROP, &[ix], true);
                            } else {
                                self.builder.emit(Op::OP_SET, &[], false);
                            }
                        }
                        _ => todo!(),
                    },
                    _ => todo!(),
                },
                PatOrExpr::Expr(e) => match &**e {
                    Expr::Member(member) => {
                        self.emit(&assign.right, true);
                        let name = if let Expr::Ident(id) = &*member.prop {
                            let s: &str = &id.sym;
                            let name = self.vm.intern(s);
                            Some(self.builder.get_sym(name))
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
                            self.builder.emit(Op::OP_SET_PROP, &[ix], true);
                        } else {
                            self.builder.emit(Op::OP_SET, &[], false);
                        }
                    }
                    _ => todo!(),
                },
            },
            Expr::Bin(binary) => {
                self.emit(&binary.right, true);
                self.emit(&binary.left, true);
                match binary.op {
                    BinaryOp::Add => {
                        self.builder.emit(Op::OP_ADD, &[], false);
                    }
                    BinaryOp::Sub => {
                        self.builder.emit(Op::OP_SUB, &[], false);
                    }
                    BinaryOp::Mul => {
                        self.builder.emit(Op::OP_MUL, &[], false);
                    }
                    BinaryOp::Div => {
                        self.builder.emit(Op::OP_DIV, &[], false);
                    }
                    BinaryOp::EqEq => {
                        self.builder.emit(Op::OP_EQ, &[], false);
                    }
                    BinaryOp::EqEqEq => self.builder.emit(Op::OP_EQ_EQ, &[], false),
                    BinaryOp::NotEq => self.builder.emit(Op::OP_NE, &[], false),
                    BinaryOp::NotEqEq => self.builder.emit(Op::OP_NE_NE, &[], false),
                    BinaryOp::Gt => self.builder.emit(Op::OP_GT, &[], false),
                    BinaryOp::GtEq => self.builder.emit(Op::OP_GE, &[], false),
                    BinaryOp::Lt => self.builder.emit(Op::OP_LT, &[], false),
                    BinaryOp::LtEq => self.builder.emit(Op::OP_LE, &[], false),
                    _ => todo!(),
                }

                if !used {
                    self.builder.emit(Op::OP_DROP, &[], false);
                }
            }
            _ => todo!(),
        }
    }

    pub fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => {
                self.emit(&expr.expr, false);
            }
            Stmt::Block(block) => {
                self.builder.emit(Op::OP_PUSH_SCOPE, &[], false);
                for stmt in block.stmts.iter() {
                    self.emit_stmt(stmt);
                }
                self.builder.emit(Op::OP_POP_SCOPE, &[], false);
            }
            Stmt::Return(ret) => {
                match ret.arg {
                    Some(ref arg) => self.emit(&**arg, true),
                    None => self.builder.emit(Op::OP_PUSH_UNDEFINED, &[], false),
                }
                self.builder.emit(Op::OP_RET, &[], false);
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
                    let sym = self.vm.intern(s);
                    let ix = *self.fmap.get(&sym).unwrap();
                    self.builder.emit(Op::OP_GET_FUNCTION, &[ix], false);
                    let nix = self.builder.get_sym(sym);
                    self.builder.emit(Op::OP_SET_VAR, &[nix], true);
                }
                _ => (),
            },

            _ => todo!(),
        }
    }
    pub fn cjmp(&mut self, cond: bool) -> impl FnOnce(&mut Self) {
        let p = self.builder.code.code.len();
        self.builder.emit(Op::OP_JMP, &[0], false);

        move |this: &mut Self| {
            let to = this.builder.code.code.len() - (p + 5);
            let ins = if cond {
                Op::OP_JMP_TRUE
            } else {
                Op::OP_JMP_FALSE
            };
            let bytes = (to as u32).to_le_bytes();
            this.builder.code.code[p] = ins as u8;
            this.builder.code.code[p + 1] = bytes[0];
            this.builder.code.code[p + 2] = bytes[1];
            this.builder.code.code[p + 3] = bytes[2];
            this.builder.code.code[p + 4] = bytes[3];
        }
    }

    pub fn jmp(&mut self) -> impl FnOnce(&mut Self) {
        let p = self.builder.code.code.len();
        self.builder.emit(Op::OP_JMP, &[0], false);

        move |this: &mut Self| {
            let to = this.builder.code.code.len() - (p + 5);
            let bytes = (to as u32).to_le_bytes();
            this.builder.code.code[p + 1] = bytes[0];
            this.builder.code.code[p + 2] = bytes[1];
            this.builder.code.code[p + 3] = bytes[2];
            this.builder.code.code[p + 4] = bytes[3];
            //this.builder.code.code[p] = ins as u8;
        }
    }
    pub fn emit_lit(&mut self, lit: &Lit) {
        match lit {
            Lit::Null(_) => self.builder.emit(Op::OP_PUSH_NULL, &[], false),
            Lit::Num(x) => {
                let val = x.value;
                if val as i32 as f64 == val {
                    self.builder
                        .emit(Op::OP_PUSH_INT, &[val as i32 as u32], false);
                } else {
                    let ix = self
                        .builder
                        .get_val(&mut self.vm, Val::Float(val.to_bits()));
                    self.builder.emit(Op::OP_PUSH_LIT, &[ix], false);
                }
            }
            Lit::Str(x) => {
                let val = x.value.to_string();
                let mut vm = self.vm;
                let ix = self.builder.get_val(&mut vm, Val::Str(val));
                self.builder.emit(Op::OP_PUSH_LIT, &[ix], false);
            }
            Lit::Bool(x) => {
                if x.value {
                    self.builder.emit(Op::OP_PUSH_TRUE, &[], false);
                } else {
                    self.builder.emit(Op::OP_PUSH_FALSE, &[], false);
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
                        let s: &str = &name.sym;
                        let name = self.vm.intern(s);
                        let ix = self.builder.get_sym(name);
                        self.emit(init, true);
                        match var.kind {
                            VarDeclKind::Let => self.builder.emit(Op::OP_DECL_LET, &[ix], true),
                            VarDeclKind::Const => {
                                self.builder.emit(Op::OP_DECL_IMMUTABLE, &[ix], true)
                            }
                            VarDeclKind::Var => {
                                //self.builder.code.var_names.push(name);
                                self.builder.emit(Op::OP_SET_VAR, &[ix], true);
                            }
                        }
                    }
                    None => {
                        let s: &str = &name.sym;
                        let name = self.vm.intern(s);
                        let ix = self.builder.get_sym(name);
                        self.builder.emit(Op::OP_PUSH_UNDEFINED, &[], false);
                        match var.kind {
                            VarDeclKind::Let => self.builder.emit(Op::OP_DECL_LET, &[ix], true),
                            VarDeclKind::Const => {
                                self.builder.emit(Op::OP_DECL_IMMUTABLE, &[ix], true)
                            }
                            VarDeclKind::Var => {
                                self.builder.emit(Op::OP_SET_VAR, &[ix], true);
                                self.builder.code.var_names.push(name);
                            }
                        }
                    }
                },
                _ => todo!(),
            }
        }
    }
}
unsafe impl Trace for Compiler {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.builder.trace(tracer);
    }
}
