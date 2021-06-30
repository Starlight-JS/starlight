use self::{frame::CallFrame, stack::Stack};
use super::ir::{BinaryOp, Trace as Ir};
use crate::letroot;
use crate::prelude::*;
use crate::vm::function::*;
use crate::vm::interpreter::*;

use crate::vm::{
    arguments::*, array::*, code_block::CodeBlock, environment::*, error::JsTypeError, error::*,
    function::JsVMFunction, native_iterator::*, object::*, slot::*, string::JsString,
    symbol_table::*, value::*, Runtime,
};
use crate::{
    bytecode::opcodes::Opcode,
    gc::{
        cell::{GcCell, GcPointer},
        snapshot::deserializer::Deserializable,
    },
};
use crate::{bytecode::*, gc::cell::Tracer};
use profile::{ArithProfile, ByValProfile};
use std::intrinsics::{likely, unlikely};
use std::ptr::null_mut;
use wtf_rs::unwrap_unchecked;

pub enum RecordResult {
    Other,
    NYI,
    TraceTooLarge,
    Ok,
}
#[inline(never)]
unsafe fn eval_record(
    rt: &mut Runtime,
    code: GcPointer<CodeBlock>,
    ip: *mut u8,
    this: JsValue,
    ctor: bool,
    scope: GcPointer<Environment>,
    callee: JsValue,
) -> Result<JsValue, JsValue> {
    let frame = rt.stack.new_frame(0, callee, scope);
    if frame.is_none() {
        let msg = JsString::new(rt, "stack overflow");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            rt, msg, None,
        )));
    }
    let mut frame = unwrap_unchecked(frame);
    (*frame).code_block = Some(code);
    (*frame).this = this;
    (*frame).env = scope;
    (*frame).ctor = ctor;
    (*frame).exit_on_return = true;
    (*frame).ip = ip;

    loop {
        let result = eval(rt, frame, &mut RecordResult::Ok, &mut vec![]);
        match result {
            Ok(value) => return Ok(value),
            Err(e) => {
                rt.stacktrace = rt.stacktrace();

                if let Some(unwind_frame) = rt.unwind() {
                    let (env, ip, sp) = (*unwind_frame).try_stack.pop().unwrap();
                    frame = unwind_frame;
                    (*frame).env = env.unwrap();
                    (*frame).ip = ip;
                    (*frame).sp = sp;
                    (*frame).push(e);
                } else {
                    return Err(e);
                }
            }
        }
    }
}
pub const MAX_TRACE_SIZE: usize = 5000;
pub unsafe fn eval(
    rt: &mut Runtime,
    frame: *mut CallFrame,
    res: &mut RecordResult,
    trace: &mut Vec<(usize, Ir)>,
) -> Result<JsValue, JsValue> {
    rt.heap().collect_if_necessary();
    let mut ip = (*frame).ip;

    let mut frame: &'static mut CallFrame = &mut *frame;
    let stack = &mut rt.stack as *mut Stack;
    let stack = &mut *stack;
    let gcstack = rt.shadowstack();
    let mut last_fast_call_ip = null_mut();
    loop {
        // if trace is too large we do not want to compile it. Just return to interpreting.
        if trace.len() > MAX_TRACE_SIZE {
            *res = RecordResult::TraceTooLarge;

            return Ok(JsValue::encode_undefined_value());
        }
        let sip = ip as usize;
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);

        stack.cursor = frame.sp;
        match opcode {
            Opcode::OP_GE0GL => {
                let index = ip.cast::<u32>().read_unaligned();
                trace.push((sip, Ir::GE0GL(index)));
                ip = ip.add(4);
                let env = frame.env;
                debug_assert!(
                    index < env.as_slice().len() as u32,
                    "invalid var index '{}' at pc: {}",
                    index,
                    ip as usize - &unwrap_unchecked(frame.code_block).code[0] as *const u8 as usize
                );

                frame.push(env.as_slice().get_unchecked(index as usize).value);
            }
            Opcode::OP_GE0SL => {
                let index = ip.cast::<u32>().read_unaligned();
                trace.push((sip, Ir::GE0SL(index)));
                ip = ip.add(4);
                let mut env = frame.env;
                debug_assert!(index < env.as_slice_mut().len() as u32);
                let val = frame.pop();
                if unlikely(!env.as_slice_mut()[index as usize].mutable) {
                    return Err(JsValue::new(
                        rt.new_type_error("Cannot assign to immutable variable".to_string()),
                    ));
                }

                env.as_slice_mut().get_unchecked_mut(index as usize).value = val;
            }
            Opcode::OP_GET_LOCAL => {
                let index = ip.cast::<u32>().read_unaligned();
                trace.push((sip, Ir::GetLocal(index)));
                ip = ip.add(4);
                let env = frame.pop().get_object().downcast::<Environment>().unwrap();
                debug_assert!(
                    index < env.as_slice().len() as u32,
                    "invalid var index '{}' at pc: {}",
                    index,
                    ip as usize - &unwrap_unchecked(frame.code_block).code[0] as *const u8 as usize
                );

                frame.push(env.as_slice().get_unchecked(index as usize).value);
            }
            Opcode::OP_SET_LOCAL => {
                let index = ip.cast::<u32>().read_unaligned();
                trace.push((sip, Ir::SetLocal(index)));
                ip = ip.add(4);
                let mut env = frame.pop().get_object().downcast::<Environment>().unwrap();
                debug_assert!(index < env.as_slice_mut().len() as u32);
                let val = frame.pop();
                if unlikely(!env.as_slice_mut()[index as usize].mutable) {
                    return Err(JsValue::new(
                        rt.new_type_error("Cannot assign to immutable variable".to_string()),
                    ));
                }

                env.as_slice_mut().get_unchecked_mut(index as usize).value = val;
            }
            Opcode::OP_GET_ENV => {
                let mut depth = ip.cast::<u32>().read_unaligned();
                trace.push((sip, Ir::GetEnv(depth)));
                ip = ip.add(4);
                let mut env = frame.env;

                while depth != 0 {
                    env = unwrap_unchecked(env.parent);
                    depth -= 1;
                }

                frame.push(JsValue::new(env));
            }
            Opcode::OP_FAST_CALL => {
                rt.heap().collect_if_necessary();
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                last_fast_call_ip = ip;
                ip = ip.offset(offset as isize);
            }
            Opcode::OP_JMP => {
                // XXX: we do not need to record jumps?
                rt.heap().collect_if_necessary();
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                ip = ip.offset(offset as isize);
            }
            Opcode::OP_JMP_IF_FALSE => {
                trace.push((sip, Ir::GuardFalse));
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let value = frame.pop();
                if !value.to_boolean() {
                    ip = ip.offset(offset as _);
                }
            }
            Opcode::OP_JMP_IF_TRUE => {
                trace.push((sip, Ir::GuardTrue));
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let value = frame.pop();
                if value.to_boolean() {
                    ip = ip.offset(offset as _);
                }
            }

            Opcode::OP_POP => {
                trace.push((sip, Ir::Pop));
                frame.pop();
            }
            Opcode::OP_PUSH_TRUE => {
                trace.push((sip, Ir::PushT));
                frame.push(JsValue::encode_bool_value(true));
            }
            Opcode::OP_PUSH_FALSE => {
                trace.push((sip, Ir::PushF));
                frame.push(JsValue::encode_bool_value(false));
            }
            Opcode::OP_PUSH_LITERAL => {
                let ix = ip.cast::<u32>().read();
                trace.push((sip, Ir::PushL(ix)));
                ip = ip.add(4);
                let constant = unwrap_unchecked(frame.code_block).literals[ix as usize];
                //assert!(constant.is_jsstring());
                frame.push(constant);
            }
            Opcode::OP_PUSH_THIS => {
                trace.push((sip, Ir::This));
                frame.push(frame.this);
            }
            Opcode::OP_PUSH_INT => {
                let int = ip.cast::<i32>().read();
                trace.push((sip, Ir::PushI(int)));
                ip = ip.add(4);
                frame.push(JsValue::encode_int32(int));
            }
            Opcode::OP_PUSH_NAN => {
                trace.push((sip, Ir::PushNaN));
                frame.push(JsValue::encode_nan_value());
            }
            Opcode::OP_PUSH_NULL => {
                trace.push((sip, Ir::PushN));
                frame.push(JsValue::encode_null_value());
            }
            Opcode::OP_FAST_RET => {
                if last_fast_call_ip.is_null(){
                    continue;
                } else {
                    ip = last_fast_call_ip;
                    last_fast_call_ip = null_mut();
                }
            }
            Opcode::OP_RET => {
                trace.push((sip, Ir::LeaveFrame));
                let mut value = if frame.sp <= frame.limit {
                    JsValue::encode_undefined_value()
                } else {
                    frame.pop()
                };

                if frame.ctor && !value.is_jsobject() {
                    value = frame.this;
                }
                let prev = rt.stack.pop_frame().unwrap();
                if prev.exit_on_return || prev.prev.is_null() {
                    return Ok(value);
                }
                frame = &mut *prev.prev;
                ip = frame.ip;

                frame.push(value);
            }
            Opcode::OP_ADD => {
                let profile = &mut *ip.cast::<ArithProfile>();
                ip = ip.add(4);

                let lhs = frame.pop();
                let rhs = frame.pop();
                profile.observe_lhs_and_rhs(lhs, rhs);
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    if let Some(val) = lhs.get_int32().checked_add(rhs.get_int32()) {
                        frame.push(JsValue::encode_int32(val));
                        continue;
                    }
                    profile.set_observed_int32_overflow();
                }
                if likely(lhs.is_number() && rhs.is_number()) {
                    let result = JsValue::new(lhs.get_number() + rhs.get_number());

                    frame.push(result);
                    continue;
                }
                #[cold]
                unsafe fn add_slowpath(
                    rt: &mut Runtime,
                    frame: &mut CallFrame,
                    lhs: JsValue,
                    rhs: JsValue,
                ) -> Result<(), JsValue> {
                    let lhs = lhs.to_primitive(rt, JsHint::None)?;
                    let rhs = rhs.to_primitive(rt, JsHint::None)?;

                    if lhs.is_jsstring() || rhs.is_jsstring() {
                        #[inline(never)]
                        fn concat(
                            rt: &mut Runtime,
                            lhs: JsValue,
                            rhs: JsValue,
                        ) -> Result<JsValue, JsValue> {
                            let lhs = lhs.to_string(rt)?;
                            let rhs = rhs.to_string(rt)?;
                            let string = format!("{}{}", lhs, rhs);
                            Ok(JsValue::encode_object_value(JsString::new(rt, string)))
                        }

                        let result = concat(rt, lhs, rhs)?;
                        frame.push(result);
                    } else {
                        let lhs = lhs.to_number(rt)?;
                        let rhs = rhs.to_number(rt)?;
                        frame.push(JsValue::new(lhs + rhs));
                    }
                    Ok(())
                }
                add_slowpath(rt, frame, lhs, rhs)?;
            }
            Opcode::OP_SUB => {
                let profile = &mut *ip.cast::<ArithProfile>();

                ip = ip.offset(4);

                let lhs = frame.pop();
                let rhs = frame.pop();

                profile.observe_lhs_and_rhs(lhs, rhs);
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    let result = lhs.get_int32().checked_sub(rhs.get_int32());
                    if likely(result.is_some()) {
                        frame.push(JsValue::encode_int32(result.unwrap()));
                        continue;
                    }
                    profile.set_observed_int32_overflow();
                }
                if likely(lhs.is_number() && rhs.is_number()) {
                    //profile.lhs_saw_number();
                    //profile.rhs_saw_number();
                    frame.push(JsValue::new(lhs.get_number() - rhs.get_number()));

                    continue;
                }
                // profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs - rhs));
            }
            Opcode::OP_DIV => {
                let profile = &mut *ip.cast::<ArithProfile>();
                ip = ip.add(4);

                let lhs = frame.pop();
                let rhs = frame.pop();
                profile.observe_lhs_and_rhs(lhs, rhs);
                if likely(lhs.is_number() && rhs.is_number()) {
                    frame.push(JsValue::new(lhs.get_number() / rhs.get_number()));
                    continue;
                }

                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs / rhs));
            }
            Opcode::OP_MUL => {
                let profile = &mut *ip.cast::<ArithProfile>();
                ip = ip.add(4);

                let lhs = frame.pop();
                let rhs = frame.pop();
                profile.observe_lhs_and_rhs(lhs, rhs);
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    let result = lhs.get_int32().checked_mul(rhs.get_int32());
                    if likely(result.is_some()) {
                        frame.push(JsValue::encode_int32(result.unwrap()));
                        continue;
                    }
                    profile.set_observed_int32_overflow();
                }
                if likely(lhs.is_number() && rhs.is_number()) {
                    frame.push(JsValue::new(lhs.get_number() * rhs.get_number()));
                    continue;
                }
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs * rhs));
            }
            Opcode::OP_REM => {
                let profile = &mut *ip.cast::<ArithProfile>();
                ip = ip.add(4);

                let lhs = frame.pop();
                let rhs = frame.pop();
                profile.observe_lhs_and_rhs(lhs, rhs);
                if likely(lhs.is_number() && rhs.is_number()) {
                    frame.push(JsValue::new(lhs.get_number() % rhs.get_number()));
                    continue;
                }
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs % rhs));
            }
            Opcode::OP_SHL => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::new((left << (right & 0x1f)) as f64));
            }
            Opcode::OP_SHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::new((left >> (right & 0x1f)) as f64));
            }

            Opcode::OP_USHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_uint32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::new((left >> (right & 0x1f)) as f64));
            }
            Opcode::OP_LESS => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    frame.push(JsValue::new(lhs.get_int32() < rhs.get_int32()));
                    continue;
                }
                frame.push(JsValue::encode_bool_value(
                    lhs.compare(rhs, true, rt)? == CMP_TRUE,
                ));
            }
            Opcode::OP_LESSEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    frame.push(JsValue::new(lhs.get_int32() <= rhs.get_int32()));
                    continue;
                }
                frame.push(JsValue::encode_bool_value(
                    rhs.compare(lhs, false, rt)? == CMP_FALSE,
                ));
            }

            Opcode::OP_GREATER => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    frame.push(JsValue::new(lhs.get_int32() > rhs.get_int32()));
                    continue;
                }
                frame.push(JsValue::encode_bool_value(
                    rhs.compare(lhs, false, rt)? == CMP_TRUE,
                ));
            }
            Opcode::OP_GREATEREQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_int32() && rhs.is_int32()) {
                    frame.push(JsValue::new(lhs.get_int32() >= rhs.get_int32()));
                    continue;
                }
                frame.push(JsValue::encode_bool_value(
                    lhs.compare(rhs, true, rt)? == CMP_FALSE,
                ));
            }
            Opcode::OP_GET_BY_ID | Opcode::OP_TRY_GET_BY_ID => {
                let name = ip.cast::<u32>().read_unaligned();
                let name = *unwrap_unchecked(frame.code_block)
                    .names
                    .get_unchecked(name as usize);
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let object = frame.pop();
                if likely(object.is_jsobject()) {
                    letroot!(obj = gcstack, object.get_jsobject());
                    #[cfg(not(feature = "no-inline-caching"))]
                    if let TypeFeedBack::PropertyCache { structure, offset } =
                        unwrap_unchecked(frame.code_block)
                            .feedback
                            .get_unchecked(fdbk as usize)
                    {
                        if GcPointer::ptr_eq(structure, &obj.structure()) {
                            frame.push(*obj.direct(*offset as _));

                            continue;
                        }
                    }

                    #[inline(never)]
                    #[cold]
                    unsafe fn slow_get_by_id(
                        rt: &mut Runtime,
                        frame: &mut CallFrame,
                        obj: &mut GcPointer<JsObject>,
                        name: Symbol,
                        fdbk: u32,
                        is_try: bool,
                    ) -> Result<(), JsValue> {
                        let mut slot = Slot::new();
                        let found = obj.get_property_slot(rt, name, &mut slot);
                        #[cfg(not(feature = "no-inline-caching"))]
                        if slot.is_load_cacheable() {
                            *unwrap_unchecked(frame.code_block)
                                .feedback
                                .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                                structure: slot
                                    .base()
                                    .unwrap()
                                    .downcast_unchecked::<JsObject>()
                                    .structure(),

                                offset: slot.offset(),
                            }
                        }
                        if found {
                            frame.push(slot.get(rt, JsValue::new(*obj))?);
                        } else {
                            if unlikely(is_try) {
                                let desc = rt.description(name);
                                return Err(JsValue::new(rt.new_reference_error(format!(
                                    "Property '{}' not found",
                                    desc
                                ))));
                            }
                            frame.push(JsValue::encode_undefined_value());
                        }
                        Ok(())
                    }
                    slow_get_by_id(
                        rt,
                        frame,
                        &mut obj,
                        name,
                        fdbk,
                        opcode == Opcode::OP_TRY_GET_BY_ID,
                    )?;
                    continue;
                }
                frame.push(get_by_id_slow(rt, name, object)?)
            }
            Opcode::OP_PUT_BY_ID => {
                let name = ip.cast::<u32>().read_unaligned();
                let name = *unwrap_unchecked(frame.code_block)
                    .names
                    .get_unchecked(name as usize);
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);

                let object = frame.pop();
                let value = frame.pop();
                if likely(object.is_jsobject()) {
                    let mut obj = object.get_jsobject();
                    'exit: loop {
                        'slowpath: loop {
                            match unwrap_unchecked(frame.code_block).feedback[fdbk as usize] {
                                #[cfg(not(feature = "no-inline-caching"))]
                                TypeFeedBack::PutByIdFeedBack {
                                    ref new_structure,
                                    ref old_structure,
                                    ref offset,
                                    ref structure_chain,
                                } => {
                                    if Some(obj.structure()) != *old_structure {
                                        break 'slowpath;
                                    }
                                    if new_structure.is_none() {
                                        *obj.direct_mut(*offset as usize) = value;
                                        break 'exit;
                                    }

                                    let vector = &structure_chain.unwrap().vector;
                                    let mut i = 0;

                                    let mut cur = old_structure.unwrap().prototype;
                                    while let Some(proto) = cur {
                                        let structure = proto.structure();
                                        if !GcPointer::ptr_eq(&structure, &vector[i]) {
                                            break 'slowpath;
                                        }
                                        i += 1;
                                        cur = structure.prototype;
                                    }

                                    *obj.direct_mut(*offset as usize) = value;
                                    break 'exit;
                                }
                                TypeFeedBack::None => {
                                    break 'slowpath;
                                }
                                _ => unreachable!(),
                            }
                        }

                        put_by_id_slow(rt, frame, &mut obj, name, value, fdbk)?;
                        break 'exit;
                    }
                    continue;
                }
            }

            Opcode::OP_CALL | Opcode::OP_TAILCALL => {
                rt.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);

                let args_start = frame.sp.sub(argc as _);

                frame.sp = args_start;
                let mut func = frame.pop();
                let mut this = frame.pop();
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);
                if unlikely(!func.is_callable()) {
                    let msg = JsString::new(rt, "not a callable object".to_string());
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                letroot!(func_object = gcstack, func.get_jsobject());
                letroot!(funcc = gcstack, *func_object);
                let func = func_object.as_function_mut();
                letroot!(args_ = gcstack, Arguments::new(this, &mut args));

                frame.ip = ip;
                stack.cursor = frame.sp;

                if func.is_vm() {
                    let vm_fn = func.as_vm_mut();
                    let scope = JsValue::new(vm_fn.scope);
                    let (this, scope) = rt.setup_for_vm_call(vm_fn, scope, &args_)?;
                    let mut exit = false;
                    if !frame.exit_on_return
                        && (opcode == Opcode::OP_TAILCALL
                            || (ip.cast::<Opcode>().read() == Opcode::OP_POP
                                && ip.add(1).cast::<Opcode>().read() == Opcode::OP_RET))
                    {
                        // rt.stack.pop_frame().unwrap();
                        exit = rt.stack.pop_frame().unwrap().exit_on_return;
                    }
                    let cframe = rt.stack.new_frame(0, JsValue::new(*funcc), scope);
                    if unlikely(cframe.is_none()) {
                        let msg = JsString::new(rt, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            rt, msg, None,
                        )));
                    }
                    let cframe = unwrap_unchecked(cframe);
                    (*cframe).code_block = Some(vm_fn.code);
                    (*cframe).this = this;

                    (*cframe).ctor = false;
                    (*cframe).exit_on_return = exit;
                    (*cframe).ip = &vm_fn.code.code[0] as *const u8 as *mut u8;

                    frame = &mut *cframe;

                    ip = (*cframe).ip;
                } else {
                    let result = func.call(rt, &mut args_, JsValue::new(*funcc))?;
                    frame.push(result);
                }
            }
            Opcode::OP_NEW | Opcode::OP_TAILNEW => {
                rt.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);

                let args_start = frame.sp.sub(argc as _);
                frame.sp = args_start;
                let mut func = frame.pop();
                let mut _this = frame.pop();
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);

                if unlikely(!func.is_callable()) {
                    let msg = JsString::new(rt, "not a callable constructor object ".to_string());
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                letroot!(func_object = gcstack, func.get_jsobject());
                letroot!(funcc = gcstack, func.get_jsobject());
                let map = func_object.func_construct_map(rt)?;
                let func = func_object.as_function_mut();
                let object = JsObject::new(rt, &map, JsObject::get_class(), ObjectTag::Ordinary);
                letroot!(
                    args_ = gcstack,
                    Arguments::new(JsValue::new(object), &mut args)
                );

                args_.ctor_call = true;
                frame.ip = ip;

                if func.is_vm() {
                    let vm_fn = func.as_vm_mut();
                    let scope = JsValue::new(vm_fn.scope);
                    let (this, scope) = rt.setup_for_vm_call(vm_fn, scope, &args_)?;
                    let mut exit = false;
                    if false && !frame.exit_on_return && (opcode == Opcode::OP_TAILNEW) {
                        // stack.pop_frame().unwrap();
                        exit = stack.pop_frame().unwrap().exit_on_return;
                    }
                    let cframe = rt.stack.new_frame(0, JsValue::new(*funcc), scope);
                    if unlikely(cframe.is_none()) {
                        let msg = JsString::new(rt, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            rt, msg, None,
                        )));
                    }

                    let cframe = unwrap_unchecked(cframe);
                    (*cframe).code_block = Some(vm_fn.code);
                    (*cframe).this = this;
                    (*cframe).ctor = true;
                    (*cframe).exit_on_return = exit;
                    (*cframe).ip = &vm_fn.code.code[0] as *const u8 as *mut u8;
                    frame = &mut *cframe;
                    ip = (*cframe).ip;
                } else {
                    let result = func.call(rt, &mut args_, JsValue::new(*funcc))?;

                    frame.push(result);
                }
            }

            Opcode::OP_DUP => {
                let v1 = frame.pop();
                frame.push(v1);
                frame.push(v1);
            }
            Opcode::OP_SWAP => {
                let v1 = frame.pop();
                let v2 = frame.pop();
                frame.push(v1);
                frame.push(v2);
            }
            Opcode::OP_NEG => {
                let v1 = frame.pop();
                if v1.is_number() {
                    frame.push(JsValue::new(-v1.get_number()));
                } else {
                    let n = v1.to_number(rt)?;
                    frame.push(JsValue::new(-n));
                }
            }

            Opcode::OP_EQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                frame.push(JsValue::encode_bool_value(lhs.abstract_equal(rhs, rt)?));
            }
            Opcode::OP_STRICTEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(lhs.strict_equal(rhs)));
            }
            Opcode::OP_NEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(!lhs.abstract_equal(rhs, rt)?));
            }
            Opcode::OP_NSTRICTEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(!lhs.strict_equal(rhs)));
            }
            Opcode::OP_PUT_BY_VAL => {
                let profile = &mut *ip.cast::<ByValProfile>();
                ip = ip.add(4);
                let object = frame.pop();
                let key = frame.pop();
                let value = frame.pop();
                profile.observe_key_and_object(key, object);
                if key.is_number() && object.is_jsobject() {
                    let index = if likely(key.is_int32()) {
                        key.get_int32() as u32
                    } else {
                        key.get_double().floor() as u32
                    };
                    let mut object = object.get_jsobject();
                    if likely(object.indexed.dense()) && likely(index < object.indexed.length()) {
                        *object.indexed.vector.at_mut(index) = value;
                        continue;
                    }
                }
                let key = key.to_symbol(rt)?;

                if likely(object.is_jsobject()) {
                    let mut obj = object.get_jsobject();
                    obj.put(rt, key, value, unwrap_unchecked(frame.code_block).strict)?;
                } else {
                    #[inline(never)]
                    unsafe fn slow(
                        rt: &mut Runtime,
                        object: JsValue,
                        key: Symbol,
                        value: JsValue,
                        strict: bool,
                    ) -> Result<JsValue, JsValue> {
                        object.to_object(rt)?.put(rt, key, value, strict)?;
                        Ok(JsValue::encode_undefined_value())
                    }

                    slow(
                        rt,
                        object,
                        key,
                        value,
                        unwrap_unchecked(frame.code_block).strict,
                    )?;
                }
            }
            Opcode::OP_GET_BY_VAL | Opcode::OP_GET_BY_VAL_PUSH_OBJ => {
                let profile = &mut *ip.cast::<ByValProfile>();
                ip = ip.add(4);

                let object = frame.pop();
                let key = frame.pop();
                profile.observe_key_and_object(key, object);
                if key.is_number() && object.is_jsobject() {
                    let index = if likely(key.is_int32()) {
                        key.get_int32() as usize
                    } else {
                        key.get_double().floor() as usize
                    };
                    let object = object.get_jsobject();
                    if likely(object.indexed.dense())
                        && likely(index < object.indexed.length() as usize)
                        && likely(!object.indexed.vector.at(index as _).is_empty())
                    {
                        if opcode == Opcode::OP_GET_BY_VAL_PUSH_OBJ {
                            frame.push(JsValue::new(object));
                        }
                        frame.push(*object.indexed.vector.at(index as _));

                        continue;
                    }
                }
                let key = key.to_symbol(rt)?;
                let mut slot = Slot::new();
                let _ = object.get_slot(rt, key, &mut slot)?;

                let value = slot.get(rt, JsValue::new(object))?;

                if opcode == Opcode::OP_GET_BY_VAL_PUSH_OBJ {
                    frame.push(JsValue::new(object));
                }
                frame.push(value);
            }
            Opcode::OP_INSTANCEOF => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(rt, "'instanceof' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                letroot!(robj = gcstack, rhs.get_jsobject());
                letroot!(robj2 = gcstack, *robj);
                if unlikely(!robj.is_callable()) {
                    let msg = JsString::new(rt, "'instanceof' requires constructor");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                frame.push(JsValue::encode_bool_value(
                    robj.as_function().has_instance(&mut robj2, rt, lhs)?,
                ));
            }
            Opcode::OP_IN => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(rt, "'in' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                let sym = lhs.to_symbol(rt)?;
                frame.push(JsValue::encode_bool_value(
                    rhs.get_jsobject().has_own_property(rt, sym),
                ));
            }

            Opcode::OP_FORIN_SETUP => {
                let offset = ip.cast::<i32>().read_unaligned();
                ip = ip.add(4);
                let enumerable = frame.pop();

                if enumerable.is_null() || enumerable.is_undefined() {
                    ip = ip.offset(offset as _);
                    frame.push(JsValue::encode_empty_value());
                    continue;
                }

                let it = if enumerable.is_jsstring() {
                    NativeIterator::new(rt, enumerable.get_object())
                } else {
                    let obj = enumerable.to_object(rt)?;
                    NativeIterator::new(rt, obj.as_dyn())
                };
                frame.push(JsValue::new(it));
                assert!(ip.cast::<Opcode>().read_unaligned() == Opcode::OP_FORIN_ENUMERATE);
            }
            Opcode::OP_FORIN_ENUMERATE => {
                let offset = ip.cast::<i32>().read_unaligned();
                ip = ip.add(4);
                let mut it = frame
                    .pop()
                    .get_object()
                    .downcast_unchecked::<NativeIterator>();
                frame.push(JsValue::new(it));
                if let Some(sym) = it.next() {
                    let desc = rt.description(sym);
                    frame.push(JsValue::new(JsString::new(rt, desc)));
                } else {
                    frame.push(JsValue::encode_empty_value());
                    ip = ip.offset(offset as _);
                }
            }
            Opcode::OP_FORIN_LEAVE => {
                frame.pop();
            }

            Opcode::OP_THROW => {
                let val = frame.pop();
                return Err(val);
            }

            Opcode::OP_GLOBALTHIS => {
                let global = rt.global_object();
                frame.push(JsValue::encode_object_value(global));
            }

            Opcode::OP_NEWOBJECT => {
                let obj = JsObject::new_empty(rt);
                frame.push(JsValue::encode_object_value(obj));
            }

            Opcode::OP_PUSH_CATCH => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let env = frame.env;

                frame
                    .try_stack
                    .push((Some(env), ip.offset(offset as isize), frame.sp));
            }
            Opcode::OP_POP_CATCH => {
                frame.try_stack.pop().unwrap();
            }

            Opcode::OP_LOGICAL_NOT => {
                let val = frame.pop();
                frame.push(JsValue::encode_bool_value(!val.to_boolean()));
            }
            Opcode::OP_NOT => {
                let v1 = frame.pop();
                if v1.is_number() {
                    let n = v1.get_number() as i32;
                    frame.push(JsValue::new((!n) as i32));
                } else {
                    let n = v1.to_number(rt)? as i32;
                    frame.push(JsValue::new((!n) as i32));
                }
            }
            Opcode::OP_POS => {
                let value = frame.pop();
                if value.is_number() {
                    frame.push(value);
                }
                let x = value.to_number(rt)?;
                frame.push(JsValue::new(x));
            }

            Opcode::OP_DECL_CONST => {
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let mut env = frame.env;
                let val = frame.pop();
                env.as_slice_mut()[ix as usize] = Variable {
                    value: val,
                    mutable: false,
                };
            }
            Opcode::OP_DECL_LET => {
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let mut env = frame.env;
                let val = frame.pop();
                env.as_slice_mut()[ix as usize] = Variable {
                    value: val,
                    mutable: true,
                };
            }

            Opcode::OP_DELETE_BY_ID => {
                let name = ip.cast::<u32>().read();
                ip = ip.add(4);
                let name = unwrap_unchecked(frame.code_block).names[name as usize];
                let object = frame.pop();
                object.check_object_coercible(rt)?;
                letroot!(object = gcstack, object.to_object(rt)?);
                frame.push(JsValue::new(object.delete(
                    rt,
                    name,
                    unwrap_unchecked(frame.code_block).strict,
                )?));
            }
            Opcode::OP_DELETE_BY_VAL => {
                let object = frame.pop();
                let name = frame.pop().to_symbol(rt)?;
                object.check_object_coercible(rt)?;
                letroot!(object = gcstack, object.to_object(rt)?);
                frame.push(JsValue::new(object.delete(
                    rt,
                    name,
                    unwrap_unchecked(frame.code_block).strict,
                )?));
            }
            Opcode::OP_AND => {
                let lhs = frame.pop().to_int32(rt)?;
                let rhs = frame.pop().to_int32(rt)?;
                frame.push(JsValue::new(lhs & rhs));
            }
            Opcode::OP_OR => {
                let lhs = frame.pop().to_int32(rt)?;
                let rhs = frame.pop().to_int32(rt)?;
                frame.push(JsValue::new(lhs | rhs));
            }
            Opcode::OP_XOR => {
                let lhs = frame.pop().to_int32(rt)?;
                let rhs = frame.pop().to_int32(rt)?;
                frame.push(JsValue::new(lhs ^ rhs));
            }
            Opcode::OP_GET_FUNCTION => {
                //vm.space().defer_gc();
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let code = unwrap_unchecked(frame.code_block).codes[ix as usize];
                let func = if likely(!(code.is_async || code.is_generator)) {
                    JsVMFunction::new(rt, code, frame.env)
                } else {
                    let func = JsVMFunction::new(rt, code, frame.env);

                    JsGeneratorFunction::new(rt, func)
                };

                frame.push(JsValue::encode_object_value(func));
                // vm.space().undefer_gc();
            }

            Opcode::OP_PUSH_UNDEF => {
                frame.push(JsValue::encode_undefined_value());
            }
            Opcode::OP_NEWARRAY => {
                let count = ip.cast::<u32>().read_unaligned();

                ip = ip.add(4);
                letroot!(arr = gcstack, JsArray::new(rt, count));
                let mut index = 0;
                let mut did_put = 0;
                while did_put < count {
                    let value = frame.pop();
                    if unlikely(value.is_object() && value.get_object().is::<SpreadValue>()) {
                        letroot!(
                            spread = gcstack,
                            value.get_object().downcast_unchecked::<SpreadValue>()
                        );
                        for i in 0..spread.array.len() {
                            let real_arg = spread.array[i];
                            arr.put(rt, Symbol::Index(index), real_arg, false)?;
                            index += 1;
                        }
                    } else {
                        arr.put(rt, Symbol::Index(index), value, false)?;
                        index += 1;
                    }
                    did_put += 1;
                }
                frame.push(JsValue::encode_object_value(*arr));
            }

            Opcode::OP_CALL_BUILTIN => {
                rt.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let builtin_id = ip.cast::<u32>().read();
                ip = ip.add(4);
                let effect = ip.cast::<u32>().read();
                ip = ip.add(4);
                crate::vm::builtins::BUILTINS[builtin_id as usize](
                    rt,
                    frame,
                    &mut ip,
                    argc,
                    effect as _,
                )?;
            }
            Opcode::OP_SPREAD => {
                /*
                    This opcode creates internal interpreter only value that is used to indicate that some argument is spread value
                    and if interpreter sees it then it tried to use `array` value from `SpreadValue`.
                    User code can't get access to this value, if it does this should be reported.
                */
                let value = frame.pop();
                let spread = SpreadValue::new(rt, value)?;
                frame.push(JsValue::encode_object_value(spread));
            }
            Opcode::OP_TYPEOF => {
                let val = frame.pop();
                let str = JsString::new(rt, val.type_of());
                frame.push(JsValue::new(str));
            }
            Opcode::OP_TO_INTEGER_OR_INFINITY | Opcode::OP_TO_LENGTH => {
                let number = frame.pop().to_number(rt)?;
                if number.is_nan() || number == 0.0 {
                    frame.push(JsValue::encode_int32(0));
                } else {
                    frame.push(JsValue::new(number.trunc()));
                }
            }
            Opcode::OP_TO_OBJECT => {
                let target = frame.pop();
                let message = frame.pop();
                if unlikely(target.is_null() || target.is_undefined()) {
                    let msg = message.to_string(rt)?;
                    return Err(JsValue::new(rt.new_type_error(msg)));
                }
                frame.push(JsValue::new(target.to_object(rt)?));
            }
            Opcode::OP_IS_CALLABLE | Opcode::OP_IS_CTOR => {
                let val = frame.pop();
                frame.push(JsValue::new(val.is_callable()));
            }
            Opcode::OP_INITIAL_YIELD => {
                frame.ip = ip;
                return Ok(JsValue::encode_undefined_value());
            }
            Opcode::OP_YIELD => {
                frame.ip = ip;
                return Ok(JsValue::encode_native_u32(FuncRet::Yield as u32));
            }
            Opcode::OP_YIELD_STAR => {
                frame.ip = ip;
                return Ok(JsValue::encode_native_u32(FuncRet::YieldStar as u32));
            }
            Opcode::OP_AWAIT => return Ok(JsValue::encode_native_u32(FuncRet::Await as u32)),
            x => {
                panic!("NYI: {:?}", x);
            }
        }
    }
}
