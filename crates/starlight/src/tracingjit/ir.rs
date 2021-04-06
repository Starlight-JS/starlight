use crate::{gc::cell::GcPointer, prelude::Structure};

pub enum Ir {
    Swap,
    PushLiteral(u32),
    PushInt(i32),
    PushTrue,
    PushFalse,
    PushUndef,
    PushNull,
    PushNaN,
    GetFunction(u32),

    Call(u32),
    New(u32),

    CallBuiltin(u32, u32, u32),
}
