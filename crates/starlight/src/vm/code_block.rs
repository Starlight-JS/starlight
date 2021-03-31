use super::value::JsValue;
use super::{symbol_table::Symbol, Runtime};
use crate::bytecode::opcodes::*;
use crate::gc::{cell::GcPointer, cell::Tracer};
use crate::{
    bytecode::TypeFeedBack,
    gc::cell::{GcCell, Trace},
    gc::snapshot::deserializer::Deserializable,
};
use std::fmt::Write;
//#[derive(GcTrace)]
pub struct CodeBlock {
    pub name: Symbol,
    pub variables: Vec<Symbol>,
    pub rest_param: Option<Symbol>,
    pub params: Vec<Symbol>,
    pub names: Vec<Symbol>,
    pub code: Vec<u8>,
    pub top_level: bool,
    pub codes: Vec<GcPointer<Self>>,
    pub literals: Vec<JsValue>,
    pub feedback: Vec<TypeFeedBack>,
    pub strict: bool,
    pub use_arguments: bool,
    pub file_name: String,
}

unsafe impl Trace for CodeBlock {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.codes.trace(visitor);
        self.literals.trace(visitor);
        self.feedback.trace(visitor);
    }
}
impl CodeBlock {
    pub fn display_to<T: Write>(&self, output: &mut T) -> std::fmt::Result {
        unsafe {
            writeln!(output, "variables: ")?;
            if self.variables.is_empty() {
                writeln!(output, " <none>")?;
            }
            for var in self.variables.iter() {
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
                let op = std::mem::transmute::<_, Opcode>(op);
                pc = pc.add(1);
                let at = pc as usize - 1 - start as usize;
                write!(output, "{:04}: ", at)?;
                match op {
                    Opcode::OP_POP => {
                        writeln!(output, "pop")?;
                    }
                    Opcode::OP_GET_BY_VAL => {
                        writeln!(output, "get_by_val",)?;
                    }
                    Opcode::OP_GET_BY_ID => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_by_id @{}, fdbk @{}", name, feedback)?;
                    }
                    Opcode::OP_PUT_BY_VAL => {
                        writeln!(output, "put_by_val ",)?;
                    }
                    Opcode::OP_PUT_BY_ID => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "put_by_id @{}, fdbk @{}", name, feedback)?;
                    }
                    Opcode::OP_PUSH_LITERAL => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "push_lit @{}", ix)?;
                    }
                    Opcode::OP_PUSH_NULL => {
                        writeln!(output, "push_null")?;
                    }
                    Opcode::OP_PUSH_UNDEF => {
                        writeln!(output, "push_undefined")?;
                    }
                    Opcode::OP_PUSH_INT => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "push_int <{}>", ix as i32)?;
                    }
                    Opcode::OP_PUSH_TRUE => {
                        writeln!(output, "push_true")?;
                    }
                    Opcode::OP_PUSH_FALSE => {
                        writeln!(output, "push_false")?;
                    }
                    Opcode::OP_GET_FUNCTION => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_function @{}", ix)?;
                    }
                    Opcode::OP_GET_VAR => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_var @{}, fdbk @{}", name, feedback)?;
                    }
                    Opcode::OP_SET_VAR => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_var @{}, fdbk @{}", name, feedback)?;
                    }

                    Opcode::OP_GET_GLOBAL => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);

                        writeln!(output, "get_global @{}", name)?;
                    }
                    Opcode::OP_SET_GLOBAL => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);

                        writeln!(output, "set_global @{}", name)?;
                    }
                    Opcode::OP_NEWOBJECT => {
                        writeln!(output, "newobject")?;
                    }
                    Opcode::OP_NEWARRAY => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "newarray <{}>", argc)?;
                    }
                    Opcode::OP_SWAP => {
                        writeln!(output, "swap")?;
                    }
                    Opcode::OP_SPREAD => writeln!(output, "spread")?,
                    Opcode::OP_CALL => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "call <{}>", argc)?;
                    }
                    Opcode::OP_CALL_BUILTIN => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let builtin = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let effect = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "call_builtin %{}, <{}> (effect %{})",
                            builtin, argc, effect
                        )?;
                    }
                    Opcode::OP_NEW => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "new <{}>", argc)?;
                    }
                    Opcode::OP_RET => {
                        writeln!(output, "ret")?;
                    }
                    Opcode::OP_ADD => {
                        writeln!(output, "add")?;
                    }
                    Opcode::OP_SUB => {
                        writeln!(output, "sub")?;
                    }
                    Opcode::OP_DIV => {
                        writeln!(output, "div")?;
                    }
                    Opcode::OP_MUL => {
                        writeln!(output, "mul")?;
                    }
                    Opcode::OP_REM => {
                        writeln!(output, "rem")?;
                    }
                    Opcode::OP_SHR => {
                        writeln!(output, "rshift")?;
                    }
                    Opcode::OP_SHL => {
                        writeln!(output, "lshift")?;
                    }
                    Opcode::OP_USHR => {
                        writeln!(output, "urshift")?;
                    }
                    Opcode::OP_EQ => {
                        writeln!(output, "eq")?;
                    }
                    Opcode::OP_STRICTEQ => {
                        writeln!(output, "stricteq")?;
                    }
                    Opcode::OP_NEQ => {
                        writeln!(output, "neq")?;
                    }
                    Opcode::OP_NSTRICTEQ => {
                        writeln!(output, "nstricteq")?;
                    }
                    Opcode::OP_GREATER => {
                        writeln!(output, "greater")?;
                    }
                    Opcode::OP_GREATEREQ => {
                        writeln!(output, "greatereq")?;
                    }
                    Opcode::OP_LESS => {
                        writeln!(output, "less")?;
                    }
                    Opcode::OP_LESSEQ => {
                        writeln!(output, "lesseq")?;
                    }
                    Opcode::OP_JMP => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Opcode::OP_FORIN_ENUMERATE => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "for_in_enumerate {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Opcode::OP_FORIN_SETUP => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "for_in_setup {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Opcode::OP_NOP => {
                        writeln!(output, "nop")?;
                    }
                    Opcode::OP_TYPEOF => {
                        writeln!(output, "typeof")?;
                    }
                    Opcode::OP_JMP_IF_FALSE => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp_if_false {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Opcode::OP_JMP_IF_TRUE => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "jmp_if_true {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }

                    Opcode::OP_PUSH_ENV => {
                        pc = pc.add(4);
                        writeln!(output, "push_scope")?;
                    }
                    /* Opcode::OP_SET_GETTER_SETTER => {
                        writeln!(output, "set_getter_setter")?;
                    }
                    Opcode::OP_SET_GETTER_SETTER_BY_ID => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_getter_setter_by_id @{}", ix)?;
                    }*/
                    Opcode::OP_POP_ENV => {
                        writeln!(output, "pop_scope")?;
                    }
                    Opcode::OP_PUSH_CATCH => {
                        let off = pc.cast::<i32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(
                            output,
                            "push_catch {}[->{}]",
                            off,
                            (pc as usize - start as usize) as i32 + off
                        )?;
                    }
                    Opcode::OP_DECL_LET => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_let @{}, fdbk @{}", name, feedback)?;
                    }

                    Opcode::OP_DECL_CONST => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_const @{}, fdbk @{}", name, feedback)?;
                    }
                    Opcode::OP_THROW => {
                        writeln!(output, "throw")?;
                    }
                    Opcode::OP_DUP => {
                        writeln!(output, "dup")?;
                    }
                    Opcode::OP_PUSH_THIS => writeln!(output, "push_this")?,
                    Opcode::OP_IN => {
                        writeln!(output, "in")?;
                    }
                    Opcode::OP_NOT => {
                        writeln!(output, "not")?;
                    }
                    Opcode::OP_LOGICAL_NOT => {
                        writeln!(output, "logical_not")?;
                    }
                    Opcode::OP_POS => {
                        writeln!(output, "positive")?;
                    }
                    Opcode::OP_PUSH_NAN => {
                        writeln!(output, "nan")?;
                    }
                    Opcode::OP_NEG => {
                        writeln!(output, "neg")?;
                    }
                    Opcode::OP_DELETE_BY_ID => {
                        let id = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "delete_by_id @{}", id)?;
                    }
                    Opcode::OP_DELETE_BY_VAL => {
                        writeln!(output, "delete")?;
                    }
                    Opcode::OP_SET_VAR_CURRENT => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_var_current @{}", name)?;
                    }
                    Opcode::OP_FORIN_LEAVE => {
                        writeln!(output, "for_in_leave")?;
                    }
                    _ => todo!("{:?}", op),
                }
            }
            Ok(())
        }
    }
    pub fn new(rt: &mut Runtime, name: Symbol, strict: bool) -> GcPointer<Self> {
        let this = Self {
            name,
            file_name: String::new(),
            strict,
            codes: vec![],
            top_level: false,
            variables: vec![],
            rest_param: None,
            params: vec![],
            names: vec![],
            code: vec![],
            use_arguments: false,
            literals: vec![],
            feedback: vec![],
        };

        rt.heap().allocate(this)
    }
}

impl GcCell for CodeBlock {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    vtable_impl!();
}
