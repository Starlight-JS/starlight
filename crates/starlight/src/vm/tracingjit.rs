//! Tracing JIT implementation.
//!
//!
//! Our JIT is very simple one. It traces execution of interpreter and then compiles trace to machine code
//! with relying only on Cranelift optimizations.

pub mod trace;
use std::collections::HashMap;

use super::interpreter::frame::CallFrame;
use super::value::*;
use trace::*;

#[repr(u32)]
pub enum TraceExitReason {
    /// When guard is failed `return_value` from trace exit is interpreted as program counter.
    GuardFailed,
    Success,
    Exception,
}

/// After 100 iterations loop is JITed
pub const DEFAULT_HOTNESS2JIT: usize = 100;
/// After 8 guard fails we blacklist loop from JITing.
pub const DEFAULT_GUARD_FAILS2BLACKLIST: usize = 8;
// After 4 trace fails we blacklist loop from JITing.
pub const DEFAULT_TRACE_FAILS2BLACKLIST: usize = 4;
/// After 150 instructions traced we stop tracing interpreter and return back to regular interpreter.
pub const DEFAULT_TRACE_LIMIT: usize = 150;
pub struct LoopInfo {
    pub hotness: usize,
    pub guard_fails: usize,
    pub trace_fails: usize,
    pub trace_id: u64,
    pub blacklisted: bool,
    /// Stores execution trace and instruction pointer.
    pub trace: Vec<(Trace, usize)>,
    pub executable_trace: Option<extern "C" fn(&mut CallFrame, return_value: *mut JsValue) -> u32>,
}

pub struct TracingJIT {
    pub loops: HashMap<(usize, usize), LoopInfo>,
    pub trace_id: u64,
}
