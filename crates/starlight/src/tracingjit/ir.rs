use crate::{
    gc::cell::GcPointer,
    vm::{code_block::CodeBlock, structure::Structure},
};

pub enum Trace {
    GE0GL(u32),
    GE0SL(u32),
    GetLocal(u32),
    SetLocal(u32),
    GetEnv(u32),
    Swap,
    Pop,
    This,
    PushI(i32),
    PushL(u32),
    PushT,
    PushF,
    PushU,
    PushN,
    PushNaN,
    GetFunction(u32),
    EnterFrame(u32),
    LeaveFrame,
    CallBuiltin(u32, u32, u32),
    NewArray(u32),
    NewObject(u32),
    Leave,
    GuardFalse,
    GuardTrue,
    PtrEq,

    BinaryIntInt(BinaryOp),
    BinaryIntNum(BinaryOp),
    BinaryNumInt(BinaryOp),
    BinaryNumNum(BinaryOp),
    BinarySlow(BinaryOp),

    NotInt,
    NotNum,
    NotSlow,
    LogicalNot,
    PosInt,
    PosNum,
    PosSlow,

    EnterCtorFrame(u32),
    LeaveCtorFrame,

    GetByIdFast(GcPointer<Structure>, u32, u32),
    PutByIddFast(GcPointer<Structure>, u32, u32),
    // slow versions of get by id and put by id. Note that
    // they still emit space for inline cache but it is set up when trace is executed.
    GetByIdSlow(u32, u32),
    PutByIdSlow(u32, u32),

    GetByValDenseIndexed,
    PutByValDenseIndexed,

    GetByValSlow,
    PutByValSlow,

    /// fails if value is not int32
    GuardInt,
    /// fails if value is not double
    GuardNumber,
    /// fails if value is not int32 and not double
    GuardAnyNumber,
    /// fails if value is not JsFunction and it is not VM function.
    GuardVMFunction(GcPointer<CodeBlock>),
    // same as above except expects native function pointer.
    GuardNativeFunction(usize),

    /// Emit native function call.
    ///
    /// This instruction will allocate storage for arguments on stack
    /// and copy arguments from VM stack to this storage, then it will
    /// invoke native function.
    ///
    CallNative(u32),
    /// Almost the same as above except this one
    /// does invoke [JsFunction::call] directly.
    CallSlow(u32),
}
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Rem,
    Shr,
    Shl,
    UShr,
    Or,
    And,
    Xor,
    In,
    Eq,
    NEq,
    StrictEq,
    NStrictEq,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    InstanceOf,
}
