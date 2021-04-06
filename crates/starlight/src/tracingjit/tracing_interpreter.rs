use crate::prelude::*;
use crate::vm::interpreter::stack::*;
use crate::vm::interpreter::*;
use crate::{bytecode::opcodes::*, vm::interpreter::frame::CallFrame};
pub enum TraceResult {
    FailedToRecord,
    Ok(JsValue),
    Err(JsValue),
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
use crate::bytecode::*;
use crate::prelude::*;
use crate::vm::code_block::*;
use crate::vm::environment::*;
use crate::vm::native_iterator::*;
use crate::vm::value::*;
use std::intrinsics::*;
use wtf_rs::unwrap_unchecked;

pub unsafe fn record(
    rt: &mut Runtime,
    frame: *mut CallFrame,
    trace: &mut Vec<Ir>,
    target: *mut u8,
    exit: &mut i32,
) -> Result<JsValue, JsValue> {
    rt.heap().collect_if_necessary();
    let mut ip = (*frame).ip;

    let mut frame: &'static mut CallFrame = &mut *frame;
    let stack = &mut rt.stack as *mut Stack;
    let stack = &mut *stack;
    let gcstack = rt.shadowstack();
    loop {
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);
        #[cfg(feature = "perf")]
        {
            rt.perf.get_perf(opcode as u8);
        }
        //println!("{:?}", opcode);
        stack.cursor = frame.sp;
        match opcode {
            Opcode::OP_NOP => {}
            Opcode::OP_GET_VAR => {
                let index = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let env = frame.pop().get_object().downcast_unchecked::<Environment>();
                debug_assert!(
                    index < env.values.len() as u32,
                    "invalid var index at pc: {}",
                    ip as usize - &unwrap_unchecked(frame.code_block).code[0] as *const u8 as usize
                );

                frame.push(env.values.get_unchecked(index as usize).0);
            }
            Opcode::OP_SET_VAR => {
                let index = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let mut env = frame.pop().get_object().downcast_unchecked::<Environment>();
                debug_assert!(index < env.values.len() as u32);
                let val = frame.pop();
                if unlikely(!env.values[index as usize].1) {
                    return Err(JsValue::new(
                        rt.new_type_error(format!("Cannot assign to immutable variable")),
                    ));
                }
                env.values.get_unchecked_mut(index as usize).0 = val;
            }
            Opcode::OP_GET_ENV => {
                let mut depth = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let mut env = frame.env.get_object().downcast_unchecked::<Environment>();

                while depth != 0 {
                    env = env.parent.expect("Invalid environment depth");
                    depth -= 1;
                }

                frame.push(JsValue::new(env));
            }

            Opcode::OP_PUSH_ENV => {
                let mut env = Environment::new(rt, 0);
                env.parent = Some(frame.env.get_object().downcast_unchecked());
                frame.env = JsValue::new(env);
            }
            Opcode::OP_POP_ENV => {
                //rt.heap().collect_if_necessary();
                let mut env = frame.env.get_object().downcast_unchecked::<Environment>();

                frame.env = JsValue::new(unwrap_unchecked(env.parent));
            }
            Opcode::OP_JMP => {
                rt.heap().collect_if_necessary();
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                ip = ip.offset(offset as isize);
            }
            Opcode::OP_JMP_IF_FALSE => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let value = frame.pop();
                if !value.to_boolean() {
                    ip = ip.offset(offset as _);
                }
            }
            Opcode::OP_JMP_IF_TRUE => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let value = frame.pop();
                if value.to_boolean() {
                    ip = ip.offset(offset as _);
                }
            }

            Opcode::OP_POP => {
                frame.pop();
            }
            Opcode::OP_PUSH_TRUE => {
                frame.push(JsValue::encode_bool_value(true));
            }
            Opcode::OP_PUSH_FALSE => {
                frame.push(JsValue::encode_bool_value(false));
            }
            Opcode::OP_PUSH_LITERAL => {
                let ix = ip.cast::<u32>().read();
                ip = ip.add(4);
                let constant = unwrap_unchecked(frame.code_block).literals[ix as usize];
                //assert!(constant.is_jsstring());
                frame.push(constant);
            }
            Opcode::OP_PUSH_THIS => {
                frame.push(frame.this);
            }
            Opcode::OP_PUSH_INT => {
                let int = ip.cast::<i32>().read();

                ip = ip.add(4);
                frame.push(JsValue::new(int as f64));
            }
            Opcode::OP_PUSH_NAN => {
                frame.push(JsValue::encode_nan_value());
            }
            Opcode::OP_PUSH_NULL => {
                frame.push(JsValue::encode_null_value());
            }
            Opcode::OP_RET => {
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
                // let profile = &mut *ip.cast::<ArithProfile>();
                // ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                // profile.observe_lhs_and_rhs(lhs, rhs);

                if likely(lhs.is_number() && rhs.is_number()) {
                    let result = JsValue::new(lhs.get_number() + rhs.get_number());

                    frame.push(result);
                    continue;
                }

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
            }
            Opcode::OP_SUB => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
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
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_number() && rhs.is_number()) {
                    //    profile.lhs_saw_number();
                    //    profile.rhs_saw_number();
                    frame.push(JsValue::new(lhs.get_number() / rhs.get_number()));
                    continue;
                }
                //profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs / rhs));
            }
            Opcode::OP_MUL => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_number() && rhs.is_number()) {
                    //  profile.lhs_saw_number();
                    //  profile.rhs_saw_number();

                    frame.push(JsValue::new(lhs.get_number() * rhs.get_number()));
                    continue;
                }
                //profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::new(lhs * rhs));
            }
            Opcode::OP_REM => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();

                if likely(lhs.is_number() && rhs.is_number()) {
                    //  profile.lhs_saw_number();
                    //  profile.rhs_saw_number();
                    frame.push(JsValue::new(lhs.get_number() % rhs.get_number()));
                    continue;
                }
                // profile.observe_lhs_and_rhs(lhs, rhs);
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

                frame.push(JsValue::encode_bool_value(
                    lhs.compare(rhs, true, rt)? == CMP_TRUE,
                ));
            }
            Opcode::OP_LESSEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(
                    rhs.compare(lhs, false, rt)? == CMP_FALSE,
                ));
            }

            Opcode::OP_GREATER => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(
                    rhs.compare(lhs, false, rt)? == CMP_TRUE,
                ));
            }
            Opcode::OP_GREATEREQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(
                    lhs.compare(rhs, true, rt)? == CMP_FALSE,
                ));
            }

            Opcode::OP_CALL => {
                rt.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let mut func = frame.pop();
                let mut this = frame.pop();

                let args_start = frame.sp.sub(argc as _);
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);
                if !func.is_callable() {
                    let msg = JsString::new(rt, "not a callable object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                root!(func_object = gcstack, func.get_jsobject());
                root!(funcc = gcstack, *&*func_object);
                let func = func_object.as_function_mut();

                root!(
                    args_ = gcstack,
                    Arguments::from_array_storage(rt, this, &mut args)
                );
                frame.ip = ip;
                frame.sp = args_start;
                if func.is_vm() {
                    let vm_fn = func.as_vm_mut();
                    let scope = JsValue::new(vm_fn.scope);
                    let (this, scope) = rt.setup_for_vm_call(vm_fn, scope, &args_)?;

                    let cframe = rt.stack.new_frame(0, JsValue::new(*funcc));
                    if cframe.is_none() {
                        let msg = JsString::new(rt, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            rt, msg, None,
                        )));
                    }

                    rt.stack.cursor = frame.sp;
                    let cframe = unwrap_unchecked(cframe);
                    (*cframe).code_block = Some(vm_fn.code);
                    (*cframe).this = this;
                    (*cframe).env = JsValue::encode_object_value(scope);
                    (*cframe).ctor = false;
                    (*cframe).exit_on_return = false;
                    (*cframe).ip = &vm_fn.code.code[0] as *const u8 as *mut u8;
                    frame = &mut *cframe;
                    ip = (*cframe).ip;
                } else {
                    let result = func.call(rt, &mut args_, JsValue::new(*funcc))?;

                    frame.push(result);
                }
            }
            Opcode::OP_NEW => {
                rt.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);

                let mut func = frame.pop();
                let mut _this = frame.pop();

                let args_start = frame.sp.sub(argc as _);
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);

                if unlikely(!func.is_callable()) {
                    let msg = JsString::new(rt, "not a callable object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                root!(func_object = gcstack, func.get_jsobject());
                root!(funcc = gcstack, func.get_jsobject());
                let map = func_object.func_construct_map(rt)?;
                let func = func_object.as_function_mut();
                let object = JsObject::new(rt, &map, JsObject::get_class(), ObjectTag::Ordinary);
                root!(
                    args_ = gcstack,
                    Arguments::from_array_storage(rt, JsValue::new(object), &mut args)
                );
                args_.ctor_call = true;
                frame.ip = ip;
                frame.sp = args_start;

                if func.is_vm() {
                    let vm_fn = func.as_vm_mut();
                    let scope = JsValue::new(vm_fn.scope);
                    let (this, scope) = rt.setup_for_vm_call(vm_fn, scope, &args_)?;

                    let cframe = rt.stack.new_frame(0, JsValue::new(*funcc));
                    if cframe.is_none() {
                        let msg = JsString::new(rt, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            rt, msg, None,
                        )));
                    }

                    rt.stack.cursor = frame.sp;
                    let cframe = unwrap_unchecked(cframe);
                    (*cframe).code_block = Some(vm_fn.code);
                    (*cframe).this = this;
                    (*cframe).env = JsValue::encode_object_value(scope);
                    (*cframe).ctor = true;
                    (*cframe).exit_on_return = false;
                    (*cframe).ip = &vm_fn.code.code[0] as *const u8 as *mut u8;
                    frame = &mut *cframe;
                    ip = (*cframe).ip;
                } else {
                    let result = func.call(rt, &mut args_, JsValue::new(*funcc))?;

                    frame.push(result);
                }
                /*let result = func.construct(rt, &mut args_, Some(map))?;

                frame.push(result);*/
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
                    root!(obj = gcstack, object.get_jsobject());
                    if likely(rt.options.inline_caches) {
                        if let TypeFeedBack::PropertyCache { structure, offset } =
                            unwrap_unchecked(frame.code_block)
                                .feedback
                                .get_unchecked(fdbk as usize)
                        {
                            if let Some(structure) = structure.upgrade() {
                                if GcPointer::ptr_eq(&structure, &obj.structure()) {
                                    frame.push(*obj.direct(*offset as _));
                                    continue;
                                }
                            }
                        }
                    }

                    let mut slot = Slot::new();
                    let found = obj.get_property_slot(rt, name, &mut slot);
                    if rt.options.inline_caches && slot.is_load_cacheable() {
                        *unwrap_unchecked(frame.code_block)
                            .feedback
                            .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                            structure: rt.heap().make_weak(
                                slot.base()
                                    .unwrap()
                                    .downcast_unchecked::<JsObject>()
                                    .structure(),
                            ),

                            offset: slot.offset(),
                        }
                    }
                    if found {
                        frame.push(slot.get(rt, object)?);
                    } else {
                        if unlikely(opcode == Opcode::OP_TRY_GET_BY_ID) {
                            let desc = rt.description(name);
                            return Err(JsValue::new(
                                rt.new_reference_error(format!("Property '{}' not found", desc)),
                            ));
                        }
                        frame.push(JsValue::encode_undefined_value());
                    }
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
                    if true {
                        if let TypeFeedBack::PropertyCache { structure, offset } =
                            unwrap_unchecked(frame.code_block)
                                .feedback
                                .get_unchecked(fdbk as usize)
                        {
                            if let Some(structure) = structure.upgrade() {
                                if GcPointer::ptr_eq(&structure, &obj.structure()) {
                                    *obj.direct_mut(*offset as usize) = value;

                                    continue;
                                }
                            }
                        }
                    }

                    let mut slot = Slot::new();

                    obj.put_slot(
                        rt,
                        name,
                        value,
                        &mut slot,
                        unwrap_unchecked(frame.code_block).strict,
                    )?;

                    if slot.is_put_cacheable() {
                        *unwrap_unchecked(frame.code_block)
                            .feedback
                            .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                            structure: rt.heap().make_weak(obj.structure()),
                            offset: slot.offset(),
                        };
                    }
                } else {
                    eprintln!("Internal waning: PUT_BY_ID on primitives is not implemented yet");
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
            Opcode::OP_INSTANCEOF => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(rt, "'instanceof' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                root!(robj = gcstack, rhs.get_jsobject());
                root!(robj2 = gcstack, *robj);
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
            Opcode::OP_PUT_BY_VAL => {
                let object = frame.pop();
                let key = frame.pop().to_symbol(rt)?;
                let value = frame.pop();
                if likely(object.is_jsobject()) {
                    let mut obj = object.get_jsobject();
                    obj.put(rt, key, value, unwrap_unchecked(frame.code_block).strict)?;
                } else {
                    eprintln!("Internal waning: PUT_BY_VAL on primitives is not implemented yet");
                }
            }
            Opcode::OP_GET_BY_VAL => {
                let object = frame.pop();
                let key = frame.pop().to_symbol(rt)?;
                let mut slot = Slot::new();
                let value = object.get_slot(rt, key, &mut slot)?;

                frame.push(value);
            }

            Opcode::OP_PUSH_CATCH => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let env = frame.env;

                frame
                    .try_stack
                    .push((env, ip.offset(offset as isize), frame.sp));
            }
            Opcode::OP_POP_CATCH => {
                frame.try_stack.pop();
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
                let mut env = frame.pop().get_object().downcast::<Environment>().unwrap();
                let val = frame.pop();
                env.values.push((val, false));
            }
            Opcode::OP_DECL_LET => {
                let mut env = frame.pop().get_object().downcast::<Environment>().unwrap();
                let val = frame.pop();
                env.values.push((val, true));
                //   println!("decl_let {}<-{}", env.values.len() - 1, val.to_string(rt)?);
            }
            Opcode::OP_DELETE_VAR => {
                todo!();
            }
            Opcode::OP_DELETE_BY_ID => {
                let name = ip.cast::<u32>().read();
                ip = ip.add(4);
                let name = unwrap_unchecked(frame.code_block).names[name as usize];
                let object = frame.pop();
                object.check_object_coercible(rt)?;
                root!(object = gcstack, object.to_object(rt)?);
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
                root!(object = gcstack, object.to_object(rt)?);
                frame.push(JsValue::new(object.delete(
                    rt,
                    name,
                    unwrap_unchecked(frame.code_block).strict,
                )?));
            }
            Opcode::OP_GET_FUNCTION => {
                //vm.space().defer_gc();
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let func = JsVMFunction::new(
                    rt,
                    unwrap_unchecked(frame.code_block).codes[ix as usize],
                    (*frame)
                        .env
                        .get_object()
                        .downcast()
                        .expect("corrupted environment"),
                );
                assert!(func.is_callable());

                frame.push(JsValue::encode_object_value(func));
                // vm.space().undefer_gc();
            }

            Opcode::OP_PUSH_UNDEF => {
                frame.push(JsValue::encode_undefined_value());
            }
            Opcode::OP_NEWARRAY => {
                let count = ip.cast::<u32>().read_unaligned();

                ip = ip.add(4);
                root!(arr = gcstack, JsArray::new(rt, count));
                let mut index = 0;
                while index < count {
                    let value = frame.pop();
                    if unlikely(value.is_object() && value.get_object().is::<SpreadValue>()) {
                        root!(
                            spread = gcstack,
                            value.get_object().downcast_unchecked::<SpreadValue>()
                        );
                        for i in 0..spread.array.get(rt, "length".intern())?.get_number() as usize {
                            let real_arg = spread.array.get(rt, Symbol::Index(i as _))?;
                            arr.put(rt, Symbol::Index(index), real_arg, false)?;
                            index += 1;
                        }
                    } else {
                        arr.put(rt, Symbol::Index(index), value, false)?;
                        index += 1;
                    }
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
            x => panic!("{:?}", x),
        }
    }
}
