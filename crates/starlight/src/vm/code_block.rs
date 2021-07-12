use super::Context;
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::value::JsValue;
use super::{symbol_table::Symbol};
use crate::bytecode::opcodes::*;
use crate::gc::{cell::GcPointer, cell::Tracer};
use crate::{
    bytecode::TypeFeedBack,
    gc::cell::{GcCell, Trace},
    gc::snapshot::deserializer::Deserializable,
};
use std::rc::Rc;
use std::{fmt::Write, ops::Range};

pub struct FileLocation {
    pub line: u32,
    pub col: u32,
}

/// A type representing single JS function bytecode.
//#[derive(GcTrace)]
#[repr(C)]
pub struct CodeBlock {
    pub literals_ptr: *const JsValue,
    /// Function name
    pub name: Symbol,
    /// Variable count
    pub var_count: u32,
    /// Parameters count
    pub param_count: u32,
    /// Rest parameter position in argument list
    pub rest_at: Option<u32>,
    /// Names
    pub names: Vec<Symbol>,
    /// Bytecode
    pub code: Vec<u8>,
    /// Is this code block a top level?
    pub top_level: bool,
    /// Functions declared inside this code block.
    pub codes: Vec<GcPointer<Self>>,
    /// Constant literals
    pub literals: Vec<JsValue>,

    /// Is this code block strict?
    pub strict: bool,
    /// Feedback vector that is used for inline caching
    pub feedback: Vec<TypeFeedBack>,

    /// Does code internally use `arguments` variable?
    pub use_arguments: bool,
    /// File name where JS code is located.
    pub file_name: String,
    /// `arguments` location in variable array.
    pub args_at: u32,

    pub is_constructor: bool,

    pub loc: Vec<(Range<usize>, FileLocation)>,
    pub path: Rc<str>,
    pub is_generator: bool,
    pub is_async: bool,
}

unsafe impl Trace for CodeBlock {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.codes.trace(visitor);
        self.literals.trace(visitor);
        self.feedback.trace(visitor);
    }
}

impl CodeBlock {
    /// Print bytecode to `output`.
    pub fn display_to<T: Write>(&self, output: &mut T) -> std::fmt::Result {
        unsafe {
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
                    Opcode::OP_GET_BY_VAL | Opcode::OP_GET_BY_VAL_PUSH_OBJ => {
                        pc = pc.add(4);
                        writeln!(output, "get_by_val",)?;
                    }
                    Opcode::OP_GET_BY_ID => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_by_id {}, fdbk {}", name, feedback)?;
                    }
                    Opcode::OP_PUT_BY_VAL => {
                        pc = pc.add(4);
                        writeln!(output, "put_by_val ",)?;
                    }
                    Opcode::OP_TRY_GET_BY_ID => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "try_get_by_id {}, fdbk {}", name, feedback)?;
                    }
                    Opcode::OP_GET_ENV => {
                        let depth = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_environment {}", depth)?;
                    }
                    Opcode::OP_SET_ENV => {
                        let depth = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "set_environment {}", depth)?;
                    }
                    Opcode::OP_PUT_BY_ID => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let feedback = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "put_by_id {}, fdbk {}", name, feedback)?;
                    }
                    Opcode::OP_PUSH_LITERAL => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "push_lit {}", ix)?;
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
                        writeln!(output, "get_function {}", ix)?;
                    }
                    Opcode::OP_GE0GL => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_environment 0->get_local {}", ix)?;
                    }
                    Opcode::OP_GE0SL => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "get_environment 0->set_local {}", ix)?;
                    }
                    Opcode::OP_GET_LOCAL => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);

                        writeln!(output, "get_local {}", name)?;
                    }
                    Opcode::OP_SET_LOCAL => {
                        let name = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);

                        writeln!(output, "set_local {}", name,)?;
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
                    Opcode::OP_TAILCALL => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "tail_call <{}>", argc)?;
                    }
                    Opcode::OP_INSTANCEOF => {
                        writeln!(output, "instanceof")?;
                    }
                    Opcode::OP_TAILNEW => {
                        let argc = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "tail_new <{}>", argc)?;
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
                        pc = pc.add(4);
                        writeln!(output, "add")?;
                    }
                    Opcode::OP_SUB => {
                        pc = pc.add(4);
                        writeln!(output, "sub")?;
                    }
                    Opcode::OP_DIV => {
                        pc = pc.add(4);
                        writeln!(output, "div")?;
                    }
                    Opcode::OP_MUL => {
                        pc = pc.add(4);
                        writeln!(output, "mul")?;
                    }
                    Opcode::OP_REM => {
                        pc = pc.add(4);
                        writeln!(output, "rem")?;
                    }
                    Opcode::OP_SHR => {
                        writeln!(output, "rshift")?;
                    }
                    Opcode::OP_IS_OBJECT => {
                        writeln!(output, "is_object")?;
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
                        writeln!(output, "set_getter_setter_by_id {}", ix)?;
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
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_let {}", ix)?;
                    }

                    Opcode::OP_DECL_CONST => {
                        let ix = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        writeln!(output, "decl_const {}", ix)?;
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
                        writeln!(output, "delete_by_id {}", id)?;
                    }
                    Opcode::OP_DELETE_BY_VAL => {
                        writeln!(output, "delete")?;
                    }

                    Opcode::OP_FORIN_LEAVE => {
                        writeln!(output, "for_in_leave")?;
                    }
                    Opcode::OP_GLOBALTHIS => {
                        writeln!(output, "global_object")?;
                    }
                    Opcode::OP_LOOPHINT => {
                        writeln!(output, "loophint")?;
                    }
                    Opcode::OP_OR => {
                        writeln!(output, "or")?;
                    }
                    Opcode::OP_AND => {
                        writeln!(output, "and")?;
                    }
                    Opcode::OP_XOR => {
                        writeln!(output, "xor")?;
                    }
                    Opcode::OP_POP_CATCH => {
                        writeln!(output, "pop_catch")?;
                    }
                    Opcode::OP_TO_OBJECT => {
                        writeln!(output, "to_object")?;
                    }
                    Opcode::OP_TO_LENGTH => {
                        writeln!(output, "to_length")?;
                    }
                    Opcode::OP_TO_INTEGER_OR_INFINITY => {
                        writeln!(output, "to_integer_or_inf")?;
                    }
                    Opcode::OP_IS_CALLABLE => {
                        writeln!(output, "is_callable")?;
                    }
                    Opcode::OP_IS_CTOR => {
                        writeln!(output, "is_constructor")?;
                    }
                    Opcode::OP_INITIAL_YIELD => writeln!(output, "initial_yield")?,
                    Opcode::OP_YIELD => writeln!(output, "yield")?,
                    Opcode::OP_YIELD_STAR => writeln!(output, "yield_star")?,
                    Opcode::OP_AWAIT => writeln!(output, "await")?,
                    _ => todo!("{:?}", op),
                }
            }
            Ok(())
        }
    }
    /// Create new empty code block.
    pub fn new(ctx: &mut Context, name: Symbol, strict: bool, path: Rc<str>) -> GcPointer<Self> {
        let this = Self {
            path,
            name,
            loc: vec![],
            file_name: String::new(),
            strict,
            codes: vec![],
            top_level: false,
            names: vec![],
            args_at: 0,
            code: vec![],
            is_constructor: true,
            rest_at: None,
            literals_ptr: core::ptr::null_mut(),
            use_arguments: false,
            literals: vec![],
            feedback: vec![],
            var_count: 0,
            param_count: 0,
            is_async: false,
            is_generator: false,
        };

        ctx.heap().allocate(this)
    }
}

impl GcCell for CodeBlock {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
