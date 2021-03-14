#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum Opcode {
    OP_NOP = 0,
    OP_PUSH_LITERAL,
    OP_PUSH_INT,
    OP_PUSH_TRUE,
    OP_PUSH_FALSE,
    OP_PUSH_UNDEF,
    OP_PUSH_NULL,
    OP_PUSH_NAN,

    OP_LOOPHINT,
    OP_CALL,
    OP_NEW,
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
    OP_POS,
    OP_NEG,
    OP_BNOT,

    OP_PUSH_CATCH,
    OP_POP_CATCH,

    OP_GET_BY_ID,
    OP_GET_BY_VAL,
    OP_PUT_BY_ID,
    OP_PUT_BY_VAL,

    OP_PUSH_ENV,
    OP_POP_ENV,
    OP_GET_ENV,
    OP_GET_VAR,
    OP_SET_VAR,
    OP_DECL_LET,
    OP_DECL_CONST,

    OP_YIELD,
    OP_NEWGENERATOR,

    OP_DUP,
    OP_POP,
    /// stack.push(Spread::new(...stack.pop()));
    OP_SPREAD,

    OP_DELETE_VAR,
    OP_DELETE_BY_ID,
    OP_DELETE_BY_VAL,
}