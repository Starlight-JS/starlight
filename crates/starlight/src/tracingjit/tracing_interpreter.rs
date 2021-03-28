use crate::prelude::*;
use crate::vm::interpreter::stack::*;
use crate::vm::interpreter::*;
use crate::{bytecode::opcodes::*, vm::interpreter::frame::CallFrame};
pub enum TraceResult {
    FailedToRecord,
    Ok(JsValue),
    Err(JsValue),
}

macro_rules! try_ {
    ($res: expr) => {
        match $res {
            Ok(val) => val,
            Err(e) => return TraceResult::Err(e),
        }
    };
}

impl std::ops::Try for TraceResult {
    type Ok = JsValue;
    type Error = JsValue;
    fn into_result(self) -> Result<<Self as std::ops::Try>::Ok, Self::Error> {
        match self {
            Self::Ok(val) => Ok(val),
            Self::Err(e) => Err(e),
            _ => unreachable!(),
        }
    }
    fn from_error(v: Self::Error) -> Self {
        Self::Err(v)
    }

    fn from_ok(v: <Self as std::ops::Try>::Ok) -> Self {
        Self::Ok(v)
    }
}

use super::ir::*;
use wtf_rs::unwrap_unchecked;
use Opcode::*;
pub const TRACE_MAX_SIZE: usize = 316;

pub unsafe fn record(rt: &mut Runtime, frame: *mut CallFrame, trace: &mut Vec<Ir>) -> TraceResult {
    rt.gc().collect_if_necessary();
    let mut ip = (*frame).ip;

    let mut frame: &'static mut CallFrame = &mut *frame;
    let stack = &mut rt.stack as *mut Stack;
    let stack = &mut *stack;
    let gcstack = rt.shadowstack();

    macro_rules! record {
        ($ir: expr) => {
            trace.push($ir);
            if trace.len() > TRACE_MAX_SIZE {
                return TraceResult::FailedToRecord;
            }
        };
    }
    loop {
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);
        stack.cursor = frame.sp;
        match opcode {
            OP_NOP => {}
            OP_GET_VAR => {
                let name_ = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let name = *unwrap_unchecked((*frame).code_block)
                    .names
                    .get_unchecked(name_ as usize);
                let value = get_var(rt, name, frame, fdbk)?;

                frame.push(value);
                record!(Ir::GetVar(name_));
            }
            OP_SET_VAR => {
                let val = frame.pop();
                let name_ = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let name = *unwrap_unchecked((*frame).code_block)
                    .names
                    .get_unchecked(name_ as usize);
                set_var(rt, frame, name, fdbk, val)?;
                record!(Ir::SetVar(name_));
            }
            OP_PUSH_ENV => {
                let _fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);

                let structure = Structure::new_indexed(rt, Some(frame.env.get_jsobject()), false);

                let env = JsObject::new(rt, &structure, JsObject::get_class(), ObjectTag::Ordinary);
                frame.env = JsValue::encode_object_value(env);
                record!(Ir::PushEnv);
            }
            OP_POP_ENV => {
                let mut env = frame.env.get_jsobject();

                frame.env = JsValue::encode_object_value(
                    env.prototype().copied().expect("no environments left"),
                );
                env.structure.prototype = None;
                record!(Ir::PopEnv);
            }

            _ => todo!(),
        }
    }
}
