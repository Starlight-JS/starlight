use std::{collections::HashMap, fmt::Write, mem::transmute, ptr::null_mut};

#[cfg(feature = "debug-snapshots")]
use serde::ser::SerializeStruct;

use crate::{
    heap::{
        cell::{Cell, Gc, Trace, Tracer},
        Allocator,
    },
    runtime::{string::JsString, structure::Structure, symbol::Symbol, value::JsValue},
    vm::VirtualMachine,
};
use minivec::mini_vec as vec;
use minivec::MiniVec as Vec;
pub mod opcodes;
use opcodes::Op;
use starlight_derive::Trace;
#[derive(Trace)]
pub struct ByteCode {
    #[unsafe_ignore_trace]
    pub name: Symbol,
    #[unsafe_ignore_trace]
    pub code: Vec<u8>,
    #[unsafe_ignore_trace]
    pub code_start: *mut u8,
    #[unsafe_ignore_trace]
    pub rest_param: Option<Symbol>,
    pub codes: Vec<Gc<ByteCode>>,
    pub feedback: Vec<TypeFeedBack>,
    pub literals: Vec<JsValue>,
    #[unsafe_ignore_trace]
    pub literals_start: *mut JsValue,
    #[unsafe_ignore_trace]
    pub names: Vec<Symbol>,
    #[unsafe_ignore_trace]
    pub params: Vec<Symbol>,
    #[unsafe_ignore_trace]
    pub strict: bool,
    #[unsafe_ignore_trace]
    pub var_names: Vec<Symbol>,
}

impl ByteCode {
    pub fn display_to<T: Write>(&self, output: &mut T) -> std::fmt::Result {
        unsafe {
            writeln!(output, "variables: ")?;
            if self.var_names.is_empty() {
                writeln!(output, " <none>")?;
            }
            for var in self.var_names.iter() {
                match var {
                    Symbol::Key(s) => {
                        writeln!(output, " var {}", s)?;
                    }
                    _ => unreachable!(),
                }
            }
            writeln!(output, "is strict?={}", self.strict)?;
            let start = self.code.as_ptr() as *mut u8;
            let mut pc = self.code.as_ptr() as *mut u8;
            while pc <= self.code.last().unwrap() as *const u8 as *mut u8 {
                let op = pc.read_unaligned();
                let op = transmute::<_, Op>(op);
                pc = pc.add(1);
                let at = pc as usize - 1 - start as usize;
                write!(output, "{:04}: ", at)?;
                match op {
                    Op::OP_DROP => {
                        writeln!(output, "drop")?;
                    }
                    Op::OP_GET => {
                        writeln!(output, "get ",)?;
                    }
                    Op::OP_GET_PROP => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_prop @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_SET => {
                        writeln!(output, "set ",)?;
                    }
                    Op::OP_SET_PROP => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_prop @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_PUSH_LIT => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "push_lit @{}", ix)?;
                    }
                    Op::OP_PUSH_NULL => {
                        writeln!(output, "push_null")?;
                    }
                    Op::OP_PUSH_UNDEFINED => {
                        writeln!(output, "push_undefined")?;
                    }
                    Op::OP_PUSH_INT => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "push_int <{}>", ix as i32)?;
                    }
                    Op::OP_PUSH_TRUE => {
                        writeln!(output, "push_true")?;
                    }
                    Op::OP_PUSH_FALSE => {
                        writeln!(output, "push_false")?;
                    }
                    Op::OP_GET_FUNCTION => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_function @{}", ix)?;
                    }
                    Op::OP_GET_VAR => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_var @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_SET_VAR => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_var @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_CREATE_OBJ => {
                        writeln!(output, "create_obj")?;
                    }
                    Op::OP_CREATE_ARRN => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "create_arrn <{}>", argc)?;
                    }
                    Op::OP_SWAP => {
                        writeln!(output, "swap")?;
                    }
                    Op::OP_SPREAD_ARR => writeln!(output, "spread_arr")?,
                    Op::OP_CALL => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "call <{}>", argc)?;
                    }
                    Op::OP_NEW => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "new <{}>", argc)?;
                    }
                    Op::OP_RET => {
                        writeln!(output, "ret")?;
                    }
                    Op::OP_ADD => {
                        writeln!(output, "add")?;
                    }
                    Op::OP_SUB => {
                        writeln!(output, "sub")?;
                    }
                    Op::OP_DIV => {
                        writeln!(output, "div")?;
                    }
                    Op::OP_MUL => {
                        writeln!(output, "mul")?;
                    }
                    Op::OP_REM => {
                        writeln!(output, "rem")?;
                    }
                    Op::OP_RSHIFT => {
                        writeln!(output, "rshift")?;
                    }
                    Op::OP_LSHIFT => {
                        writeln!(output, "lshift")?;
                    }
                    Op::OP_URSHIFT => {
                        writeln!(output, "urshift")?;
                    }
                    Op::OP_EQ => {
                        writeln!(output, "eq")?;
                    }
                    Op::OP_EQ_EQ => {
                        writeln!(output, "stricteq")?;
                    }
                    Op::OP_NE => {
                        writeln!(output, "neq")?;
                    }
                    Op::OP_NE_NE => {
                        writeln!(output, "strictneq")?;
                    }
                    Op::OP_GT => {
                        writeln!(output, "gt")?;
                    }
                    Op::OP_GE => {
                        writeln!(output, "ge")?;
                    }
                    Op::OP_LT => {
                        writeln!(output, "lt")?;
                    }
                    Op::OP_LE => {
                        writeln!(output, "le")?;
                    }
                    Op::OP_JMP => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Op::OP_NOP => {
                        writeln!(output, "nop")?;
                    }
                    Op::OP_TYPEOF => {
                        writeln!(output, "typeof")?;
                    }
                    Op::OP_JMP_FALSE => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp_if_false {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Op::OP_JMP_TRUE => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp_if_true {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Op::OP_PUSH_EMPTY => {
                        writeln!(output, "push_empty")?;
                    }
                    Op::OP_PUSH_SCOPE => {
                        writeln!(output, "push_scope")?;
                    }
                    Op::OP_SET_GETTER_SETTER => {
                        writeln!(output, "set_getter_setter")?;
                    }
                    Op::OP_SET_GETTER_SETTER_BY_ID => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_getter_setter_by_id @{}", ix)?;
                    }
                    Op::OP_POP_SCOPE => {
                        writeln!(output, "pop_scope")?;
                    }
                    Op::OP_TRY_PUSH_CATCH => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "try_push_catch {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Op::OP_DECL_LET => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_let @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_PLACEHOLDER => {
                        writeln!(output, "placeholder")?;
                    }
                    Op::OP_DECL_IMMUTABLE => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_immutable @{}, fdbk @{}", name, feedback)?;
                    }
                    Op::OP_THROW => {
                        writeln!(output, "throw")?;
                    }
                    Op::OP_DUP => {
                        writeln!(output, "dup")?;
                    }
                    Op::OP_PUSH_THIS => writeln!(output, "push_this")?,
                    Op::OP_IN => {
                        writeln!(output, "in")?;
                    }
                    _ => todo!("{:?}", op),
                }
            }
            Ok(())
        }
    }
    pub fn new(vm: &mut VirtualMachine, name: Symbol, params: &[Symbol], strict: bool) -> Gc<Self> {
        vm.allocate(Self {
            name,
            var_names: vec![],
            rest_param: None,
            code: vec![],
            code_start: null_mut(),
            codes: vec![],
            feedback: vec![],
            literals: vec![],
            literals_start: null_mut(),
            names: vec![],
            params: Vec::from(params),
            strict,
        })
    }
}

impl Cell for ByteCode {}
#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for ByteCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("ByteCode", 2)?;
        x.serialize_field("code", &self.code)?;
        x.serialize_field("is_strict", self.strict)?;
        x.end()
    }
}
pub enum TypeFeedBack {
    Generic,
    Structure(
        Gc<Structure>,
        u32, /* field offset */
        u32, /* number of ICs happened */
    ),
    None,
    X,
}
unsafe impl Trace for TypeFeedBack {
    fn trace(&self, tracer: &mut dyn Tracer) {
        match self {
            Self::Structure(ref x, _, _) => x.trace(tracer),
            _ => (),
        }
    }
}

pub struct ByteCodeBuilder {
    pub code: Gc<ByteCode>,
    pub name_map: HashMap<Symbol, u32>,
    pub val_map: HashMap<Val, u32>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Val {
    Float(u64),
    Str(String),
}
impl ByteCodeBuilder {
    pub fn finish(&mut self, vm: &mut VirtualMachine) -> Gc<ByteCode> {
        self.code.code_start = &mut self.code.code[0];
        if vm.options.dump_bytecode {
            let mut buf = String::new();
            let name = vm.description(self.code.name);
            self.code.display_to(&mut buf).unwrap();
            eprintln!("Code block '{}' at {:p}: \n {}", name, self.code.cell, buf);
        }
        self.code
    }
    pub fn new(vm: &mut VirtualMachine, name: Symbol, params: &[Symbol], strict: bool) -> Self {
        Self {
            code: ByteCode::new(vm, name, params, strict),
            val_map: Default::default(),
            name_map: Default::default(),
        }
    }
    pub fn get_val(&mut self, vm: &mut VirtualMachine, val: Val) -> u32 {
        if let Some(ix) = self.val_map.get(&val) {
            return *ix;
        }

        let val_ = match val.clone() {
            Val::Float(x) => JsValue::new(f64::from_bits(x)),
            Val::Str(x) => JsValue::new(JsString::new(vm, x)),
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
    pub fn emit(&mut self, op: opcodes::Op, operands: &[u32], add_feedback: bool) {
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
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.code.trace(tracer);
    }
}
