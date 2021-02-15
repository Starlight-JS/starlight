use cranelift::prelude::*;
use std::ops::{Deref, DerefMut};
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
#[repr(C)]
pub enum Op {
    /// Removes an item from the top of the stack. It is undefined what happens if
    /// the stack is empty.
    ///
    /// `( a -- )`
    OP_DROP = 0,

    /// Duplicates a value on top of the stack.
    ///
    /// `( a -- a a)`
    OP_DUP,

    /// Duplicates 2 values from the top of the stack in the same order.
    ///
    /// `( a b -- a b a b)`
    OP_2DUP,

    /// Swap the top two items on the stack.
    ///
    /// `( a b -- b a )`
    OP_SWAP,

    /// Copy current top of the stack to the temporary stash register.
    ///
    /// The content of the stash register will be cleared in the event of an
    /// exception.
    ///
    /// `( a S: b -- a S: a)` saves TOS to stash reg
    OP_STASH,

    /// Replace the top of the stack with the content of the temporary stash
    /// register.
    ///
    /// The stash register is cleared afterwards.
    ///
    /// `( a S: b -- b S: nil )` replaces tos with stash reg
    OP_UNSTASH,

    /// Effectively drops the last-but-one element from stack
    ///
    /// `( a b -- b )`
    OP_SWAP_DROP,

    /// Pushes `undefined` onto the stack.
    ///
    /// `( -- undefined )`
    OP_PUSH_UNDEFINED,

    /// Pushes `null` onto the stack.
    ///
    /// `( -- null )`
    OP_PUSH_NULL,

    /// Pushes current value of `this` onto the stack.
    ///
    /// `( -- this )`
    OP_PUSH_THIS,

    /// Pushes `true` onto the stack.
    ///
    /// `( -- true )`
    OP_PUSH_TRUE,

    /// Pushes `false` onto the stack.
    ///
    /// `( -- false )`
    OP_PUSH_FALSE,

    /// Pushes `0` onto the stack.
    ///
    /// `( -- 0 )`
    OP_PUSH_ZERO,

    /// Pushes `1` onto the stack.
    ///
    /// `( -- 1 )`
    OP_PUSH_ONE,

    /// Pushes a value from literals table onto the stack.
    ///
    /// The opcode takes a varint operand interpreted as an index in the current
    /// literal table (see lit table).
    ///
    /// ( -- a )
    OP_PUSH_LIT,

    OP_NOT,
    OP_LOGICAL_NOT,

    /// Takes a number from the top of the stack, inverts the sign and pushes it
    /// back.
    ///
    /// `( a -- -a )`
    OP_NEG,

    /// Takes a number from the top of the stack pushes the evaluation of
    /// `Number()`.
    ///
    /// `( a -- Number(a) )`
    OP_POS,

    /// Takes 2 values from the top of the stack and performs addition operation:
    /// If any of the two values is not `undefined`, number or boolean, both values
    /// are converted into strings and concatenated.
    /// Otherwise, both values are treated as numbers:
    /// /// `undefined` is converted into NaN
    /// /// `true` is converted into 1
    /// /// `false` is converted into 0
    ///
    /// Result is pushed back onto the stack.
    ///
    /// TODO: make it behave exactly like JavaScript's `+` operator.
    ///
    /// `( a b -- a+b )`
    OP_ADD,
    OP_SUB,     //// ( a b -- a-b )
    OP_REM,     //// ( a b -- a%b )
    OP_MUL,     //// ( a b -- a///b )
    OP_DIV,     //// ( a b -- a/b )
    OP_LSHIFT,  //// ( a b -- a<<b )
    OP_RSHIFT,  //// ( a b -- a>>b )
    OP_URSHIFT, //// ( a b -- a>>>b )
    OP_OR,      //// ( a b -- a|b )
    OP_XOR,     //// ( a b -- a^b )
    OP_AND,     //// ( a b -- a&b )

    /// Takes two numbers form the top of the stack and pushes `true` if they are
    /// equal, or `false` if they are not equal.
    ///
    /// ( a b -- a===b )
    OP_EQ_EQ,
    OP_EQ,    //// ( a b -- a==b )
    OP_NE,    //// ( a b -- a!=b )
    OP_NE_NE, //// ( a b -- a!==b )
    OP_LT,    //// ( a b -- a<b )
    OP_LE,    //// ( a b -- a<=b )
    OP_GT,    //// ( a b -- a>b )
    OP_GE,    //// ( a b -- a>=b )
    OP_INSTANCEOF,

    OP_TYPEOF,

    OP_IN,

    /// Takes 2 values from the stack, treats the top of the stack as property name
    /// and the next value must be an object, an array or a string.
    /// If it's an object, pushes the value of its named property onto the stack.
    /// If it's an array or a string, returns a value at a given position.
    OP_GET,

    /// Takes 3 items from the stack: value, property name, object. Sets the given
    /// property of a given object to a given value, pushes value back onto the
    /// stack.
    ///
    ///
    /// `( a b c -- a[b]=c )`
    OP_SET,

    /// Takes 1 value from the stack and a varint argument -- index of the var name
    /// in the literals table. Tries to find the variable in the current scope
    /// chain and assign the value to it. If the variable is not found -- creates
    /// a new one in the global scope. Pushes the value back to the stack.
    ///
    ///
    /// `( a -- a )`
    OP_SET_VAR,

    /// Takes a varint argument -- index of the var name in the literals table.
    /// Looks up that variable in the scope chain and pushes its value onto the
    /// stack.
    ///
    ///
    /// `( -- a )`
    OP_GET_VAR,

    /// Like OP_GET_VAR but returns undefined
    /// instead of throwing reference error.
    ///
    ///
    /// `( -- a )`
    OP_SAFE_GET_VAR,

    // ==== Jumps
    //
    //
    // All jump instructions take one 4-byte argument: offset to jump to. Offset is a
    // index of the byte in the instruction stream, starting with 0. No byte order
    // conversion is applied.
    //
    // TODO: specify byte order for the offset.
    /// Unconditiona jump.
    OP_JMP,

    /// Takes one value from the stack and performs a jump if conversion of that
    /// value to boolean results in `true`.
    ///
    /// `( a -- )`
    OP_JMP_TRUE,

    /// Takes one value from the stack and performs a jump if conversion of that
    /// value to boolean results in `false`.
    ///
    /// `( a -- )`
    OP_JMP_FALSE,

    /// Like OP_JMP_TRUE but if the branch
    /// is taken it also drops another stack element:
    ///
    /// if `b` is true: `( a b -- )`
    /// if `b` is false: `( a b -- a )`
    OP_JMP_TRUE_DROP,

    /// Conditional jump on the v7->is_continuing flag.
    /// Clears the flag once executed.
    ///
    /// `( -- )`
    OP_JMP_IF_CONTINUE,

    /// Constructs a new empty object and pushes it onto the stack.
    ///
    /// `( -- {} )`
    OP_CREATE_OBJ,

    /// Constructs a new empty array and pushes it onto the stack.
    ///
    /// `( -- [] )`
    OP_CREATE_ARR,

    /// Allocates the iteration context (for `OP_NEXT_PROP`) from heap and pushes
    /// a foreign pointer to it on stack. The allocated data is stored as "user
    /// data" of the object, and it will be reclaimed automatically when the
    /// object gets garbage-collected.
    ///
    /// `( -- ctx )`
    OP_PUSH_PROP_ITER_CTX,

    /// Yields the next property name.
    /// Used in the for..in construct.
    ///
    /// The first evaluation must receive `null` as handle.
    /// Subsequent evaluations will either:
    ///
    /// a) produce a new handle, the key and true value:
    ///
    /// `( o h -- o h' key true)`
    ///
    /// b) produce a false value only, indicating no more properties:
    ///
    /// `( o h -- false)`
    OP_NEXT_PROP,

    /// Copies the function object at TOS and assigns current scope
    /// in func->scope.
    ///
    /// `( a -- a )`
    OP_FUNC_LIT,

    /// Takes the number of arguments as parameter.
    ///
    /// Pops N function arguments from stack, then pops function, then pops `this`.
    /// Calls a function and populates TOS with the returned value.
    ///
    /// `( this f a0 a1 ... aN -- f(a0,a1,...) )`
    OP_CALL,
    OP_NEW,

    /// Checks that TOS is a callable and if not saves an exception
    /// that will will be thrown by CALL after all arguments have been evaluated.
    OP_CHECK_CALL,

    /// Returns the current function.
    ///
    /// It has no stack side effects. The function upon return will leave the
    /// return value on the stack. The return value must be pushed on the stack
    /// prior to invoking a RET.
    ///
    /// `( -- )`
    OP_RET,

    /// Deletes the property of given name `p` from the given object `o`. Returns
    /// boolean value `a`.
    ///
    /// `( o p -- a )`
    OP_DELETE,

    /// Like `OP_DELETE`, but uses the current scope as an object to delete
    /// a property from.
    ///
    /// `( p -- a )`
    OP_DELETE_VAR,

    /// Pushes a value (bcode offset of `catch` block) from opcode argument to
    /// "try stack".
    ///
    /// Used in the beginning of the `try` block.
    ///
    /// `( A: a -- T: a )`
    OP_TRY_PUSH_CATCH,

    /// Pushes a value (bcode offset of `finally` block) from opcode argument to
    /// "try stack".
    ///
    /// Used in the beginning of the `try` block.
    ///
    /// `( A: a -- T: a )`
    ///
    /// TODO: implement me
    OP_TRY_PUSH_FINALLY,

    /// Pushes a value (bcode offset of a label) from opcode argument to
    /// "try stack".
    ///
    /// Used at the beginning of loops that contain break or continue.
    /// Possible optimisation: don't emit if we can ensure that no break or
    /// continue statement is used.
    ///
    /// `( A: a -- T: a )`
    OP_TRY_PUSH_LOOP,

    /// Pushes a value (bcode offset of a label) from opcode argument to
    /// "try stack".
    ///
    /// Used at the beginning of switch statements.
    ///
    /// `( A: a -- T: a )`
    OP_TRY_PUSH_SWITCH,

    /// Pops a value (bcode offset of `finally` or `catch` block) from "try
    /// stack", and discards it
    ///
    /// Used in the end of the `try` block, as well as in the beginning of the
    /// `catch` and `finally` blocks
    ///
    /// `( T: a -- T: )`
    OP_TRY_POP,

    /// Used in the end of the `finally` block:
    ///
    /// - if some value is currently being thrown, keep throwing it.
    ///   If eventually we encounter `catch` block, the thrown value gets
    ///   populated on TOS:
    ///
    ///   `( -- a )`
    ///
    /// - if there is some pending value to return, keep returning it.
    ///   If we encounter no further `finally` blocks, then the returned value
    ///   gets populated on TOS:
    ///
    ///   `( -- a )`
    ///
    ///   And return is performed.
    ///
    /// - otherwise, do nothing
    OP_AFTER_FINALLY,

    /// Throw value from TOS. First of all, it pops the value and saves it into
    /// `v7->vals.thrown_error`:
    ///
    /// `( a -- )`
    ///
    /// Then unwinds stack looking for the first `catch` or `finally` blocks.
    ///
    /// - if `finally` is found, thrown value is kept into `v7->vals.thrown_error`.
    /// - if `catch` is found, thrown value is pushed back to the stack:
    ///   `( -- a )`
    /// - otherwise, thrown value is kept into `v7->vals.thrown_error`
    OP_THROW,

    /// Unwind to next break entry in the try stack, evaluating
    /// all finally blocks on its way up.
    ///
    /// `( -- )`
    OP_BREAK,

    /// Like OP_BREAK, but sets the v7->is_continuing flag
    /// which will cause OP_JMP_IF_CONTINUE to restart the loop.
    ///
    /// `( -- )`
    OP_CONTINUE,

    /// Used when we enter the `catch` block. Takes a varint argument -- index of
    /// the exception variable name in the literals table.
    ///
    /// Pops the exception value from the stack, creates a private frame,
    /// sets exception property on it with the given name. pushes this
    /// private frame to call stack.
    ///
    /// `( e -- )`
    OP_ENTER_CATCH,

    /// Ued when we exit from the `catch` block. Merely pops the private frame
    /// from the call stack.
    ///
    /// `( -- )`
    OP_EXIT_CATCH,

    OP_MAX,
}

/// This value is 2^49, used to encode doubles such that the encoded value will begin
/// with a 15-bit pattern within the range 0x0002..0xFFFC.
pub const DOUBLE_ENCODE_OFFSET_BIT: usize = 49;
pub const DOUBLE_ENCODE_OFFSET: u64 = 1 << DOUBLE_ENCODE_OFFSET_BIT as u64;
pub const NUMBER_TAG: u64 = 0xfffe000000000000;
pub const LOWEST_OF_HIGH_BITS: u64 = 1 << 49;
pub const OTHER_TAG: u64 = 0x2;
pub const BOOL_TAG: u64 = 0x4;
pub const UNDEFINED_TAG: u64 = 0x8;
pub const VALUE_FALSE: u64 = OTHER_TAG | BOOL_TAG | 0;
pub const VALUE_TRUE: u64 = OTHER_TAG | BOOL_TAG | 1;
pub const VALUE_UNDEFINED: u64 = OTHER_TAG | UNDEFINED_TAG;
pub const VALUE_NULL: u64 = OTHER_TAG;
pub const MISC_TAG: u64 = OTHER_TAG | BOOL_TAG | UNDEFINED_TAG;
// NOT_CELL_MASK is used to check for all types of immediate values (either number or 'other').
pub const NOT_CELL_MASK: u64 = NUMBER_TAG | OTHER_TAG;

/// These special values are never visible to JavaScript code; Empty is used to represent
/// Array holes, and for uninitialized JsValues. Deleted is used in hash table code.
/// These values would map to cell types in the JsValue encoding, but not valid GC cell
/// pointer should have either of these values (Empty is null, deleted is at an invalid
/// alignment for a GC cell, and in the zero page).
pub const VALUE_EMPTY: u64 = 0x0;
pub const VALUE_DELETED: u64 = 0x4;
pub struct InterpreterCompiler<'a> {
    pub builder: &'a mut FunctionBuilder<'a>,
}

impl InterpreterCompiler<'_> {
    pub fn fall(&mut self, params: &[types::Type], vals: &[Value]) {
        let bb = self.create_block();
        for ty in params.iter() {
            self.append_block_param(bb, *ty);
        }
        self.ins().fallthrough(bb, vals);
        self.switch_to_block(bb);
    }
    pub fn undefined_value(&mut self) -> Value {
        self.builder
            .ins()
            .iconst(types::I64, VALUE_UNDEFINED as i64)
    }

    pub fn empty_value(&mut self) -> Value {
        self.builder.ins().iconst(types::I64, VALUE_EMPTY as i64)
    }

    pub fn null_value(&mut self) -> Value {
        self.builder.ins().iconst(types::I64, VALUE_NULL as i64)
    }

    pub fn false_value(&mut self) -> Value {
        self.builder.ins().iconst(types::I64, VALUE_FALSE as i64)
    }

    pub fn is_empty(&mut self, val: Value) -> Value {
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, val, VALUE_EMPTY as i64)
    }

    pub fn is_undefined(&mut self, val: Value) -> Value {
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, val, VALUE_UNDEFINED as i64)
    }

    pub fn is_null(&mut self, val: Value) -> Value {
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, val, VALUE_NULL as i64)
    }

    pub fn is_false(&mut self, val: Value) -> Value {
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, val, VALUE_FALSE as i64)
    }
    pub fn is_true(&mut self, val: Value) -> Value {
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, val, VALUE_TRUE as i64)
    }

    pub fn as_int32(&mut self, val: Value) -> Value {
        self.builder.ins().ireduce(types::I32, val)
    }

    pub fn as_boolean(&mut self, val: Value) -> Value {
        self.is_true(val)
    }

    pub fn is_undefined_or_null(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, UNDEFINED_TAG as i64);
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, x, VALUE_NULL as i64)
    }

    pub fn is_boolean(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, !1i64);
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, x, VALUE_FALSE as i64)
    }

    pub fn is_cell(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, NOT_CELL_MASK as i64);
        self.builder.ins().icmp_imm(IntCC::Equal, x, 0)
    }

    pub fn is_int32(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, NUMBER_TAG as i64);
        self.builder
            .ins()
            .icmp_imm(IntCC::Equal, x, NUMBER_TAG as i64)
    }

    pub fn is_number(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, NUMBER_TAG as i64);
        self.builder.ins().icmp_imm(IntCC::NotEqual, x, 0)
    }

    pub fn as_double(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .iadd_imm(val, -(DOUBLE_ENCODE_OFFSET as i64));
        self.builder.ins().bitcast(types::F64, x)
    }
    pub fn new_double(&mut self, val: Value) -> Value {
        let x = self.builder.ins().bitcast(types::I64, val);
        self.builder.ins().iadd_imm(x, DOUBLE_ENCODE_OFFSET as i64)
    }
    pub fn as_cell(&mut self, val: Value) -> Value {
        val
    }

    pub fn new_int_const(&mut self, x: i32) -> Value {
        /* let y = self.builder.ins().iconst(types::I64, NUMBER_TAG as i64);
        self.builder.ins().bor_imm(y, x as i64)*/
        self.builder
            .ins()
            .iconst(types::I64, NUMBER_TAG as i64 | x as i64)
    }

    pub fn new_int(&mut self, val: Value) -> Value {
        let val = self.builder.ins().sextend(types::I64, val);
        self.builder.ins().bor_imm(val, NUMBER_TAG as i64)
    }

    pub fn binary_op_with_overflow_check(
        &mut self,
        to: Block,
        args: &[Value],
        closure: impl FnOnce(&mut Self) -> (Value, Value),
    ) -> Value {
        let res = closure(self);
        let bb = self.builder.create_block();
        self.builder.ins().brif(IntCC::Overflow, res.1, to, args);
        self.builder.ins().fallthrough(bb, args);
        self.builder.switch_to_block(bb);

        res.0
    }

    pub fn generate_loop(&mut self, op: Value, frame: Value, sp: Value, vm: Value) {
        let mut table = JumpTableData::with_capacity(32);

        macro_rules! def_block {
            ($($name: ident),*)=> {
                 $(
                    let $name = self.create_block();
                   /*  self.append_block_param($name,types::I64); // op
                    self.append_block_param($name,types::I64); // frame
                    self.append_block_param($name,types::I64);
                    self.append_block_param($name,types::I64);
                    */
                    table.push_entry($name);
                )*
            }
        }

        def_block!(
            op_drop,
            op_dup,
            op_2dup,
            op_swap,
            op_stash,
            op_unstash,
            op_swap_drop,
            op_push_undefined,
            op_push_null,
            op_push_this,
            op_push_true,
            op_push_false,
            op_push_zero,
            op_push_one,
            op_push_lit,
            op_not,
            op_logical_not,
            op_neg,
            op_pos,
            op_add,
            op_sub,
            op_rem,
            op_mul,
            op_div,
            op_lshift,
            op_rshift,
            op_urshift,
            op_or,
            op_xor,
            op_and,
            op_eq_eq,
            op_eq,
            op_ne,
            op_ne_ne,
            op_lt,
            op_le,
            op_gt,
            op_ge,
            op_instanceof,
            op_typeof,
            op_in,
            op_get,
            op_set,
            op_set_var,
            op_get_var,
            op_safe_get_var,
            op_jmp,
            op_jmp_true,
            op_jmp_false,
            op_jmp_true_drop,
            op_jmp_if_continue,
            op_create_obj,
            op_create_arr,
            op_push_prop_iter_ctx,
            op_next_prop,
            op_func_lit,
            op_call,
            op_new,
            op_check_call,
            op_ret,
            op_delete,
            op_delete_var,
            op_try_push_catch,
            op_try_push_finally,
            op_try_push_loop,
            op_try_push_switch,
            op_try_pop,
            op_after_finally,
            op_throw,
            op_break,
            op_continue,
            op_enter_catch,
            op_exit_catch
        );
        let table = self.create_jump_table(table);
        macro_rules! get_params {
            ($name : ident) => {{
                let p = self.block_params($name);
                (p[0], [1], p[2], p[3])
            }};
        }
        macro_rules! dispatch {
            (%op: expr,$frame: expr,$sp: expr,$vm: expr) => {};
        }
        {
            self.switch_to_block(op_drop);
        }
    }
    pub fn last(&mut self, addr: Value) -> Value {
        self.ins().load(types::I64, MemFlags::new(), addr, -8)
    }
    pub fn pop(&mut self, addr: Value) -> (Value, Value) {
        let imm = self.builder.ins().iconst(types::I8, 8);
        let new_sp = self.builder.ins().isub(addr, imm);
        let val = self
            .builder
            .ins()
            .load(types::I64, MemFlags::new(), new_sp, 0);
        (val, new_sp)
    }

    pub fn push(&mut self, addr: Value, val: Value) -> Value {
        self.builder.ins().store(MemFlags::new(), addr, val, 0);
        self.builder.ins().iadd_imm(addr, 8)
    }
}

impl<'a> Deref for InterpreterCompiler<'a> {
    type Target = FunctionBuilder<'a>;
    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl<'a> DerefMut for InterpreterCompiler<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.builder
    }
}
