/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
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
    pub fn analyze_module_items(items: &[ModuleItem]) -> Self {
        let mut scope = Self {
            vars: Default::default(),
            symbols: Default::default(),
        };
        let mut path = vec![];

        for stmt in items {
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
    fn visit_arrow_expr(&mut self, _n: &ArrowExpr, _: &dyn Node) {
        // self.with(ScopeKind::Arrow, |a| n.visit_children_with(a))
    }

    /// Overriden not to add ScopeKind::Block
    fn visit_block_stmt_or_expr(&mut self, _n: &BlockStmtOrExpr, _: &dyn Node) {
        // println!("Boob");
        // match n {
        //     BlockStmtOrExpr::BlockStmt(s) => s.stmts.visit_with(n, self),
        //     BlockStmtOrExpr::Expr(e) => e.visit_with(n, self),
        // }
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
    fn visit_function(&mut self, _n: &Function, _: &dyn Node) {
        return;
    }

    fn visit_fn_decl(&mut self, n: &FnDecl, _: &dyn Node) {
        self.declare(BindingKind::Function, &n.ident);
        return;
    }

    fn visit_fn_expr(&mut self, n: &FnExpr, _: &dyn Node) {
        if let Some(ident) = &n.ident {
            self.declare(BindingKind::Function, ident);
        }
        return;
    }

    fn visit_class_decl(&mut self, _n: &ClassDecl, _: &dyn Node) {
        return;
    }

    fn visit_block_stmt(&mut self, _n: &BlockStmt, _: &dyn Node) {
        return;
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
