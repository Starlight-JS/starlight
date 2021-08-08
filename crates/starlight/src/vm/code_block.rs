use comet::internal::finalize_trait::FinalizeTrait;

use super::context::Context;
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::symbol_table::Symbol;
use super::value::JsValue;
use crate::bytecode::opcodes::*;
use crate::gc::{cell::GcPointer, cell::Visitor};
use crate::{
    bytecode::TypeFeedBack,
    gc::cell::{GcCell, Trace},
};
use std::rc::Rc;
use std::{fmt::Write, ops::Range};

pub struct FileLocation {
    pub line: u32,
    pub col: u32,
}

#[derive(Default)]
struct StackSizeState {
    bc_len: u32,
    stack_len_max: u32,
    stack_level_tab: Vec<u16>,
    pc_stack: Vec<u32>,
}

impl StackSizeState {
    pub fn check(
        &mut self,
        ctx: GcPointer<Context>,
        pos: u32,
        op: u8,
        stack_len: u32,
    ) -> Result<(), JsValue> {
        if pos >= self.bc_len {
            return Err(JsValue::new(ctx.new_range_error(format!(
                "bytecode buffer overflow (op={:x} pc={:4})",
                op, pos,
            ))));
        }
        if stack_len > self.stack_len_max {
            self.stack_len_max = stack_len;
            if self.stack_len_max > ctx.stack_len_max() {
                return Err(JsValue::new(ctx.new_range_error("stack overflow")));
            }
        }
        if self.stack_level_tab[pos as usize] != 0xffff {
            if self.stack_level_tab[pos as usize] != stack_len as u16 {
                /* return Err(JsValue::new(ctx.new_range_error(format!(
                    "unconsistent stack size: {} (op={:x} pc={:4})",
                    self.stack_level_tab[pos as usize], op, pos,
                ))));*/
                panic!(
                    "unconsistent stack size: {}, expected: {} (op={:?} pc={:4})",
                    self.stack_level_tab[pos as usize],
                    stack_len,
                    unsafe { std::mem::transmute::<_, Opcode>(op) },
                    pos,
                );
            } else {
                return Ok(());
            }
        }
        self.stack_level_tab[pos as usize] = stack_len as u16;
        self.pc_stack.push(pos);
        Ok(())
    }
}
/// A type representing single JS function bytecode.
//#[derive(GcTrace)]
#[repr(C)]
pub struct CodeBlock {
    pub stack_size: u32,
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

impl Trace for CodeBlock {
    fn trace(&self, visitor: &mut Visitor) {
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
            writeln!(output, "stack size={}", self.stack_size)?;
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
                    Opcode::OP_ENTER_CATCH => {
                        writeln!(output, "enter_catch")?;
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
    pub fn compute_stack_size(&mut self, mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let mut stack_len;
        let mut s = StackSizeState::default();
        let mut pos;
        let mut op: Opcode;
        s.stack_level_tab = vec![0xffff; self.code.len()];
        s.bc_len = self.code.len() as _;
        // breath first graph exploration
        s.check(ctx, 0, Opcode::OP_NOP as u8, 0)?;
        use Opcode::*;
        while !s.pc_stack.is_empty() {
            pos = s.pc_stack.pop().unwrap();
            stack_len = s.stack_level_tab[pos as usize];
            op = unsafe { std::mem::transmute::<_, Opcode>(self.code[pos as usize]) };
            let mut skip_check = false;
            pos += 1;
            match op {
                OP_PUSH_NAN | OP_PUSH_NULL | OP_PUSH_THIS | OP_PUSH_TRUE | OP_PUSH_UNDEF
                | OP_PUSH_FALSE => stack_len += 1,
                OP_PUSH_INT | OP_GET_FUNCTION | OP_PUSH_LITERAL => {
                    pos += 4;
                    stack_len += 1;
                }
                OP_CALL | OP_NEW => {
                    let p = pos as usize;
                    let argc = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= argc as u16;
                    stack_len -= 2;

                    stack_len += 1;
                }
                OP_TAILCALL | OP_TAILNEW => {
                    let p = pos as usize;
                    let argc = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= argc as u16;
                    stack_len -= 2;
                    skip_check = true;
                }
                OP_RET | OP_THROW => {
                    stack_len -= 1;
                    skip_check = true;
                }
                OP_GET_BY_VAL => {
                    pos += 4; // SKIP PROFILE
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_GET_BY_VAL_PUSH_OBJ => {
                    pos += 4;
                    stack_len -= 2;
                    stack_len += 2;
                }
                OP_PUT_BY_VAL => {
                    pos += 4;
                    stack_len -= 3;
                }
                OP_GREATEREQ | OP_GREATER | OP_INSTANCEOF | OP_IN | OP_NSTRICTEQ | OP_NEQ
                | OP_LESS | OP_LESSEQ | OP_USHR | OP_SHR | OP_SHL | OP_EQ | OP_STRICTEQ => {
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_DUP => {
                    stack_len += 1;
                }
                OP_PUT_BY_ID => {
                    pos += 8;
                    stack_len -= 2;
                }
                OP_GET_BY_ID | OP_TRY_GET_BY_ID => {
                    pos += 8;
                    stack_len -= 1;
                    stack_len += 1;
                }
                OP_REM | OP_MUL | OP_DIV | OP_SUB | OP_ADD => {
                    pos += 4;
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_POP => {
                    stack_len -= 1;
                }
                OP_JMP => {
                    let p = pos as usize;
                    let diff = i32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    pos = (pos as i32 + diff) as u32;
                }
                OP_JMP_IF_FALSE | OP_JMP_IF_TRUE => {
                    let p = pos as usize;
                    let diff = i32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= 1;
                    s.check(ctx, (pos as i32 + diff) as u32, op as _, stack_len as _)?;
                }
                OP_ENTER_CATCH => {
                    stack_len += 1;
                }
                OP_PUSH_CATCH => {
                    let p = pos as usize;
                    let diff = i32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;

                    s.check(ctx, (pos as i32 + diff) as u32, op as _, stack_len as _)?;
                }
                OP_POP_CATCH => {}
                OP_GET_ENV => {
                    pos += 4;
                    stack_len += 1;
                }
                OP_SET_LOCAL => {
                    pos += 4;
                    stack_len -= 2;
                }
                OP_GET_LOCAL => {
                    pos += 4;
                }
                OP_GE0SL => {
                    pos += 4;
                    stack_len -= 1;
                }
                OP_GE0GL => {
                    pos += 4;
                    stack_len += 1;
                }
                OP_FORIN_SETUP => {
                    let p = pos as usize;
                    let diff = i32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    s.check(ctx, (pos as i32 + diff) as u32, op as _, stack_len as _)?;
                }
                OP_FORIN_ENUMERATE => {
                    let p = pos as usize;
                    let diff = i32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= 1;
                    stack_len += 1;
                    s.check(ctx, (pos as i32 + diff) as u32, op as _, stack_len as _)?;
                    stack_len += 1;
                }
                OP_FORIN_LEAVE => {
                    stack_len -= 1;
                }
                OP_GLOBALTHIS => {
                    stack_len += 1;
                }
                OP_NEWOBJECT => {
                    stack_len += 1;
                }
                OP_LOGICAL_NOT => {}
                OP_NOT => {}
                OP_POS => {}
                OP_DECL_CONST => {
                    pos += 4;
                    stack_len -= 1;
                }
                OP_DECL_LET => {
                    pos += 4;
                    stack_len -= 1;
                }
                OP_DELETE_BY_ID => {
                    pos += 4;
                }
                OP_DELETE_BY_VAL => {
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_AND | OP_OR | OP_XOR => {
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_NEWARRAY => {
                    let p = pos as usize;
                    let diff = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= diff as u16;
                    stack_len += 1;
                }
                OP_CALL_BUILTIN => {
                    let p = pos as usize;
                    let _argc = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    let p = pos as usize;
                    let id = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    let p = pos as usize;
                    let _effect = u32::from_ne_bytes([
                        self.code[p],
                        self.code[p + 1],
                        self.code[p + 2],
                        self.code[p + 3],
                    ]);
                    pos += 4;
                    stack_len -= crate::vm::builtins::BUILTIN_ARGS[id as usize] as u16;
                    stack_len += 1;
                }
                OP_SPREAD => {}
                OP_TYPEOF => {}
                OP_TO_INTEGER_OR_INFINITY | OP_TO_LENGTH => {}
                OP_TO_OBJECT => {
                    stack_len -= 2;
                    stack_len += 1;
                }
                OP_IS_CALLABLE | OP_IS_CTOR => {}
                OP_INITIAL_YIELD | OP_YIELD | OP_YIELD_STAR => {}
                OP_AWAIT => {}
                OP_IS_OBJECT => {}
                _ => (),
            }
            if stack_len > s.stack_len_max as u16 {
                s.stack_len_max = stack_len as _;
            }
            if !skip_check {
                s.check(ctx, pos, op as _, stack_len as _)?;
            }
        }
        self.stack_size = s.stack_len_max;
        Ok(())
    }
    /// Create new empty code block.
    pub fn new(
        mut ctx: GcPointer<Context>,
        name: Symbol,
        strict: bool,
        path: Rc<str>,
    ) -> GcPointer<Self> {
        let this = Self {
            path,
            name,
            stack_size: 0,
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

impl GcCell for CodeBlock {}
impl FinalizeTrait<CodeBlock> for CodeBlock {}
