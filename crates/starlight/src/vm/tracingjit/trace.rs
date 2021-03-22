use crate::gc::cell::*;

/// TracingJIT trace.
///
///
/// All opcodes that ends with `Slow` in name always invoke runtime stub to perform operation. Others
/// inline everything to native machine code.
///
pub enum Trace {
    /// Guard that function is callable. Most of the times inserted before [Trace::SlowCall](Trace::SlowCall)
    GuardIsCallable,
    /// Guard that compares values by pointer.
    GuardPtrEq(GcPointer<dyn GcCell>),
    GuardNumber,
    GuardBoolean,
    GuardUndefined,
    GuardNull,
    GuardTrue,
    GuardFalse,

    AddJSNumber,
    SubJSNumber,
    MulJSNumber,
    DivJSNumber,
    RemJSNumber,
    GreaterJSNumber,
    GreaterEqJSNumber,
    LessJSNumber,
    LessEqJSNumber,
    EqJSNumber,
    NeqJSNumber,
    StrictEqJSNumber,
    StrictNEqJSNumber,

    LessString,
    LessEqString,
    GreaterEqString,
    GreaterString,

    EqPtr,
    NeqPtr,
    StrictEqPtr,
    NStrictEqPtr,

    AddSlow,
    SubSlow,
    MulSlow,
    DivSlow,
    RemSlow,
    GreaterSlow,
    GreaterEqSlow,
    LessSlow,
    LessEqSlow,
    EqSlow,
    NeqSlow,
    StrictEqSlow,
    StrictNEqSlow,
    /// "slow" call is call that invokes runtime stub to perform call.
    ///
    /// This instruction does not do any guard about function type before calling it.
    SlowCall,
    /// Fast call. This in fact does not emit call instruction but just saves some of variables before entering
    /// inlined function body.
    FastCall,
    /// Restore some variables after return from inlined function
    FastReturn,

    Return,
    InSlow,
    InstanceOfSlow,
    /// Emits fast path with simple check `obj.structure() == structure` and then slow path which invokes runtime stub
    GetById(u32, u32, GcPointer<dyn GcCell>),
    /// Emits fast path with simple check `obj.structure() == structure` and then slow path which invokes runtime stub
    PutById(u32, u32, GcPointer<dyn GcCell>),
    /// Emit call to runtime stub. Note that this in fact does inline caching too.
    GetByIdSlow(u32),
    /// Emit call to runtime stub. Note that this in fact does inline caching too.
    PutByIdSlow(u32),

    GetVarFast(u32, u32, GcPointer<dyn GcCell>),
    SetVarFast(u32, u32, GcPointer<dyn GcCell>),

    GetVarSlow(u32, u32),
    SetVarSlow(u32, u32),

    GetByVal,
    PutByVal,

    DeclLetSlow(u32, u32),
    DeclConstSlow(u32, u32),
    DeleteVarSlow(u32),
}
