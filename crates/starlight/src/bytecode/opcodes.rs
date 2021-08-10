/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # Starlight's VM bytecode definition.
//!
//! Bytecode instruction consists of 1-byte opcode, optionally followed by N 32 bit operands.
//!
//!
//!  Stack diagrams follow the syntax and semantics of: [Forth stack diagrams](http://everything2.com/title/Forth+stack+diagrams).
//!
//!
//! We use the following extension in terminology:
//! - `T`: try stack
//! - `A`: opcode operands aka arguments
//!
//!
//!
//!
//! # Opcode descritpion
//!
//! - **nop**:
//!     no-operation
//! - **swap**: Swap the top two items on the stack. `(a b -- b a)`
//! - **push_literal**: Pushes a value from literal table onto the stack.
//!
//!     
//!     The opcode has 1 operand that is index to literal table in code block.
//!
//!
//!     `(-- a)`
//! - **push_int**: Pushes a new int32 value onto the stack.
//!
//!     
//!     The opcode has 1 int32 operand.
//!
//!
//!     `(-- a)`
//!
//! - **push_true**: Pushes `true` value onto the stack.
//!
//!
//!     `(-- a)`
//! - **push_false**: Pushes `false` value onto the stack.
//!
//!
//!     `(-- a)`
//!
//! - **push_undef**: Pushes `undefined` value onto the stack.
//!
//!
//!     `(-- a)`
//! - **push_null**: Pushes `null` value onto the stack.
//!
//!
//!     `(-- a)`
//!
//! - **push_nan**: Pushes `NaN` value onto the stack.
//!
//!
//!     `(-- a)`
//!
//! - **get_function**: Pushes function from code block function table onto the stack.
//!
//!
//!     Opcode has 1 operand that is index to function table.
//!
//!
//!     `(-- a)`
//!
//! - **loophint**: no-op opcode indicating loop body start.
//! - **call**: Takes the number of arguments as parameter.
//!
//!
//!     Pops N function arguments from stack, then pops function, then pops `this`.
//!     Calls a function and pushes result to the stack.
//!
//!     
//!     `( this f a0 a1 ... aN -- f(a0,a1,...) )`  
//!  - **tailcall**: same as **call** but pops the current call frame.
//!  - **new**: same as **call** but invokes object constructor.
//!  - **tailnew**: same as **tailcall** but works like **new**.
//!  - **call_builtin**: call to builtin function.
//!
//!     Has 1 operand, index of builtin. Stack is manipulated in builtins.
//!  - **newarray**: Takes the number of arguments from the stack and creates new array instance.
//!
//!
//!     Pops N arguments from stack and creates array instance.
//!
//!
//!     `( a0 a1 ... aN -- [a0,a1,...])`
//!  - **newobject**: Pushes new empty object onto the stack.
//!
//!     
//!     `( -- a )`
//!
//! - **ret**: Returns value from the current function.
//!
//!     If stack is empty will return `undefined` value, if constructor was called
//!     will try to pop value from stack and if it is object return it, otherwise will return `this` value.
//!
//!  - **jmp**: Unconditional jump.
//!  - **jmp_if_true**: Takes one value from the stack and performs a jump if conversion of
//!     that values to boolean results in `true`.
//!
//!     `( a -- )`
//!
//!  - **jmp_if_false**: Takes one value from the stack and performs a jump if conversion of
//!     that values to boolean results in `false`.
//!
//!     `( a -- )`
//!  ## Binary operations
//! Takes 2 values from the stack and performs corresponding actions to execute binary operation.
//!
//! `( a b -- a <op> b)
//!  - **add**
//!  - **sub**
//!  - **div**
//!  - **mul**
//!  - **rem**
//!  - **shr**
//!  - **shl**
//!  - **ushr**
//!  - **or**
//!  - **and**
//!  - **xor**
//!  - **in**
//!  - **eq**
//!  - **stricteq**
//!  - **neq**
//!  - **nstricteq**
//!  - **greater**
//!  - **greatereq**
//!  - **less**
//!  - **lesseq**
//!  - **instanceof**
//!
//!
//! - **typeof**: Takes a value from stack and pushes its type as string to the stack.
//!
//!  `(a -- typeof(a))`
//!
//! ## Unary operations
//! Takes 1 value from the stack and performs unary operation pushing result onto the stack.
//!
//!  
//! `(a -- <op> a)`
//!
//! - **not**
//! - **logical_not**
//! - **pos**
//! - **new**
//!
//!
//! - **throw**: Takes a value from the stack and throws it.
//!
//!
//!    `(a -- )
//!
//! - **push_catch**: Push catch block address to catch stack.
//! - **pop_catch**: Pop catch block address from catch_stack.
//!
//! - **get_by_id**: Takes object from the stack and loads the value by ID.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum Opcode {
    OP_NOP = 0,
    OP_SWAP,
    OP_PUSH_LITERAL,
    OP_PUSH_INT,
    OP_PUSH_TRUE,
    OP_PUSH_FALSE,
    OP_PUSH_UNDEF,
    OP_PUSH_NULL,
    OP_PUSH_NAN,
    OP_GET_FUNCTION,

    OP_LOOPHINT,
    OP_CALL,
    OP_TAILCALL,
    OP_TAILNEW,
    OP_NEW,
    OP_CALL_BUILTIN,
    OP_NEWARRAY,
    OP_NEWOBJECT,
    OP_RET,
    OP_JMP,
    OP_JMP_IF_TRUE,
    OP_JMP_IF_FALSE,

    OP_ADD,
    OP_SUB,
    OP_DIV,
    OP_MUL,
    OP_REM,
    OP_SHR,
    OP_SHL,
    OP_USHR,
    OP_OR,
    OP_AND,
    OP_XOR,
    OP_IN,
    OP_EQ,
    OP_STRICTEQ,
    OP_NEQ,
    OP_NSTRICTEQ,
    OP_GREATER,
    OP_GREATEREQ,
    OP_LESS,
    OP_LESSEQ,
    OP_INSTANCEOF,

    OP_TYPEOF,
    OP_NOT,
    OP_LOGICAL_NOT,
    OP_POS,
    OP_NEG,
    OP_THROW,
    OP_PUSH_CATCH,
    OP_POP_CATCH,
    OP_ENTER_CATCH,
    OP_GET_BY_ID,
    OP_TRY_GET_BY_ID,
    OP_GET_BY_VAL,
    OP_GET_BY_VAL_PUSH_OBJ,
    OP_PUT_BY_ID,
    OP_PUT_BY_VAL,

    OP_PUSH_ENV,
    OP_POP_ENV,
    OP_GET_ENV,
    OP_SET_ENV,
    OP_GET_LOCAL,
    OP_SET_LOCAL,
    OP_SET_GLOBAL,
    OP_GET_GLOBAL,
    OP_DECL_LET,
    OP_DECL_CONST,
    OP_PUSH_THIS,

    OP_DUP,
    OP_POP,
    /// stack.push(Spread::new(...stack.pop()));
    OP_SPREAD,

    OP_DELETE_VAR,
    OP_DELETE_BY_ID,
    OP_DELETE_BY_VAL,
    OP_GLOBALTHIS,

    OP_FORIN_SETUP,
    OP_FORIN_ENUMERATE,
    OP_FORIN_LEAVE,

    OP_FOROF_SETUP,
    OP_FOROF_ENUMERATE,
    OP_FOROF_LEAVE,

    /// get_env 0 get_local
    OP_GE0GL,
    /// get_env 0 set_local
    OP_GE0SL,
    /// get_env 0 decl_let
    OP_GE0DL,
    /// get_env 0 decl_const
    OP_GE0DC,

    OP_TO_OBJECT,
    OP_TO_LENGTH,
    OP_TO_INTEGER_OR_INFINITY,
    OP_IS_CALLABLE,
    OP_IS_CTOR,

    // Almost the same as OP_YIELD except returns undefined from interpreter loop.
    OP_INITIAL_YIELD,
    OP_YIELD,
    OP_YIELD_STAR,
    OP_AWAIT,
    OP_NEWGENERATOR,
    OP_IS_OBJECT,
}

pub type RegisterId = u16;

pub enum OpCode {
    Move(RegisterId, RegisterId),
    GetLocal(RegisterId, u16),
    SetLocal(RegisterId, u16),
    GetConstant(RegisterId, u16),
    GetFunction(RegisterId, u16),
    Swap(RegisterId, RegisterId),
    LoadInt(RegisterId, i32),
    LoadDouble(RegisterId, f64),
    LoadFalse(RegisterId),
    LoadTrue(RegisterId),
    LoadNull(RegisterId),
    LoadUndefined(RegisterId),
    LoadNaN(RegisterId),
    /// Call
    ///
    /// argv[0] - function
    /// argv[1] - this
    /// argv[..argc] - arguments
    ///
    Call {
        receiver: RegisterId,
        argv: RegisterId,
        argc: u16,
    },
    /// Tail call
    ///
    /// argv[0] - function
    /// argv[1] - this
    /// argv[..argc] - arguments
    TailCall {
        argv: RegisterId,
        argc: u16,
    },
    /// Call constructor
    ///
    /// argv[0] - object
    /// argv[..argc] - arguments
    CallConstructor {
        argv: RegisterId,
        argc: u16,
    },

    CreateArray {
        dest: RegisterId,
        size: u32,
    },
    CreateArrayUnknownSize {
        dest: RegisterId,
    },
    CreateObject {
        dest: RegisterId,
    },
    CreateObjectWithStructure {
        dest: RegisterId,
        /// Index to structures vector.
        structure: u16,
    },

    Return {
        source: RegisterId,
    },

    Jump {
        offset: i32,
    },
    JumpIfTrue {
        source: RegisterId,
        offset: i32,
    },
    JumpIfFalse {
        source: RegisterId,
        offset: i32,
    },

    Add {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Sub {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Div {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Mul {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Rem {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Shr {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Shl {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    UShr {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Or {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    And {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Xor {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    In {
        src_dest: RegisterId,
        rhs: RegisterId,
    },
    Equal {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    NotEqual {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    StrictEq {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    StrictNEq {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Greater {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    GreaterOrEqual {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    Less {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    LessOrEqual {
        dest: RegisterId,
        lhs: RegisterId,
        rhs: RegisterId,
    },
    InstanceOf {
        src_dest: RegisterId,
        rhs: RegisterId,
    },
    TypeOf {
        src_dest: RegisterId,
    },
    Not {
        dest: RegisterId,
        src: RegisterId,
    },
    LogicalNot {
        dest: RegisterId,
        src: RegisterId,
    },
    UnaryPlus {
        dest: RegisterId,
        src: RegisterId,
    },
    Negate {
        dest: RegisterId,
        src: RegisterId,
    },
    Throw {
        value: RegisterId,
    },
    EnterTry {
        catch_offset: i32,
    },
    LeaveTry,
    TryGetById {
        dst: RegisterId,
        object: RegisterId,
        prop: u32,
        fdbk: u32,
    },
    GetById {
        dst: RegisterId,
        object: RegisterId,
        prop: u32,
        fdbk: u32,
    },
    PutById {
        dst: RegisterId,
        object: RegisterId,
        value: RegisterId,
        prop: u32,
        fdbk: u32,
    },

    PutByVal {
        dst: RegisterId,
        object: RegisterId,
        key: RegisterId,
        value: RegisterId,
    },
    GetByVal {
        dst: RegisterId,
        object: RegisterId,
        key: RegisterId,
    },

    GetEnvironment {
        dst: RegisterId,
        depth: i16,
    },
    SetEnvironment {
        src: RegisterId,
        depth: i16,
    },

    GetVar {
        dst: RegisterId,
        env: RegisterId,
        at: u16,
    },
    SetVar {
        src: RegisterId,
        env: RegisterId,
        at: u16,
    },
    GetGlobal {
        dst: RegisterId,
    },

    DeclLet {
        env: RegisterId,
        at: u16,
    },
    DeclConst {
        env: RegisterId,
        at: u16,
    },
    DeclVar {
        env: RegisterId,
        at: u16,
    },

    LoadThis {
        dst: RegisterId,
    },

    DeleteVar {
        dst: RegisterId,
        env: RegisterId,
        at: u16,
    },
    DeleteById {
        dst: RegisterId,
        object: RegisterId,
        prop: u32,
    },
    DeleteByVal {
        dst: RegisterId,
        object: RegisterId,
        key: RegisterId,
    },

    ToObject {
        dst: RegisterId,
        src: RegisterId,
    },
    ToLength {
        dst: RegisterId,
        src: RegisterId,
    },
    ToIntOrInf {
        dst: RegisterId,
        src: RegisterId,
    },
    IsCallable {
        dst: RegisterId,
        src: RegisterId,
    },
    IsCtor {
        dst: RegisterId,
        src: RegisterId,
    },
    InitialYield,
    Yield {
        src_dest: RegisterId,
    },
    YieldStar {
        src_dest: RegisterId,
    },
    Await {
        src_dest: RegisterId,
    },
    IsObject {
        dst: RegisterId,
        src: RegisterId,
    },
}
