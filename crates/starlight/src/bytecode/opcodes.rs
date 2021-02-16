//! Starlight bytecode defintion. Most of opcodes is borrowed from v7 engines although a lot of
//! opcodes was added for handling exceptions and scopes better.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum Op {
    /// Removes an item from the top of the stack. It is undefined what happens if
    /// the stack is empty.
    ///
    /// `( a -- )`
    OP_DROP = 0,

    /// For internal usage in the bytecode compiler, should never be emitted.
    OP_PLACEHOLDER,

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
    OP_PUSH_INT,
    /// Pushes a value from literals table onto the stack.
    ///
    /// The opcode takes a varint operand interpreted as an index in the current
    /// literal table (see lit table).
    ///
    /// ( -- a )
    OP_PUSH_LIT,
    OP_PUSH_EMPTY,
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

    OP_GET_PROP,
    OP_SET_PROP,

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

    OP_GET_SCOPE,
    OP_POP_SCOPE,
    OP_PUSH_SCOPE,
    OP_DECL_VAR,
    OP_DECL_IMMUTABLE,
    OP_DECL_LET,
    OP_GET_FUNCTION,
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
