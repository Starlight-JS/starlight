use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    bytecode::{
        block::BytecodeBlock,
        opcodes::{OpCode, RegisterId},
    },
    gc::GcPointer,
    vm::{
        symbol_table::{Internable, Symbol},
        value::JsValue,
    },
};
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

use self::register_scope::RegisterScope;
pub mod register_scope;
pub struct Generator {
    pub block: GcPointer<BytecodeBlock>,
    pub parent: *mut Self,
    pub registers: RegisterScope,
    pub scope: ScopeRef,
}
impl Generator {
    pub fn fdbk(&mut self) -> u32 {
        let ix = self.block.compiled_mut().feedback.len();
        self.block
            .compiled_mut()
            .feedback
            .push(crate::bytecode::TypeFeedBack::None);
        ix as _
    }
    pub fn emit(&mut self, opcode: OpCode) {
        self.block.compiled_mut().code.push(opcode);
    }

    pub fn lookup_scope(&self, var: Symbol) -> Option<(u16, ScopeRef)> {
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

    pub fn access_var(&self, var: Symbol) -> Access {
        if let Some((ix, scope)) = self.lookup_scope(var) {
            let cur_depth = self.scope.borrow().depth;
            let depth = cur_depth - scope.borrow().depth;
            Access::Variable(ix, depth)
        } else {
            Access::Global(var)
        }
    }
}
pub trait Visit {
    type Result;
    fn visit(&self, generator: &mut Generator) -> Self::Result;
    fn visit_mut(&mut self, generator: &mut Generator) -> Self::Result {
        self.visit(generator)
    }
}

impl Visit for swc_ecmascript::ast::Expr {
    type Result = Result<RegisterId, JsValue>;
    fn visit(&self, generator: &mut Generator) -> Self::Result {
        match self {
            Self::Lit(literal) => literal.visit(generator),
            _ => todo!(),
        }
    }
}
/*
impl Visit for swc_ecmascript::ast::Ident {
    type Result = Result<RegisterId, JsValue>;
    fn visit(&self, generator: &mut Generator) -> Self::Result {
        let result = generator.lookup_scope(self.sym.intern());
        let dest = generator.registers.allocate(true);
        match result {
            Some((var, scope)) => {

            }
            None => {
                generator.emit(OpCode::GetGlobal { dst: dest });
                generator.emit(OpCode::TryGetById {
                    dst: dest,
                    object: dest,
                    prop: 0,
                    fdbk: 0,
                });
            }
        }
        Ok(dest)
    }
}*/

impl Visit for swc_ecmascript::ast::Lit {
    type Result = Result<RegisterId, JsValue>;
    fn visit(&self, generator: &mut Generator) -> Self::Result {
        match self {
            swc_ecmascript::ast::Lit::Bool(x) => {
                let dst = generator.registers.allocate(true);
                if x.value {
                    generator.emit(OpCode::LoadTrue(dst))
                } else {
                    generator.emit(OpCode::LoadFalse(dst))
                }
                return Ok(dst);
            }
            swc_ecmascript::ast::Lit::Num(x) => {
                let dst = generator.registers.allocate(true);
                if x.value as i32 as f64 == x.value {
                    generator.emit(OpCode::LoadInt(dst, x.value as i32));
                } else {
                    generator.emit(OpCode::LoadDouble(dst, x.value));
                }
                return Ok(dst);
            }
            _ => todo!(),
        }
    }
}
