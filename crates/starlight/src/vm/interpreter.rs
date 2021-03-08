use self::frame::CallFrame;
use super::{
    arguments::*, array::*, array_storage::ArrayStorage, attributes::*, code_block::CodeBlock,
    error::JsTypeError, error::*, function::JsVMFunction, function::*, object::*,
    property_descriptor::*, slot::*, string::JsString, structure::*, symbol_table::*, value::*,
    Runtime,
};
use crate::bytecode::*;
use crate::{
    bytecode::opcodes::Opcode,
    heap::{
        cell::{GcCell, GcPointer, Trace},
        SlotVisitor,
    },
};
use std::{
    hint::unreachable_unchecked,
    intrinsics::{likely, unlikely},
    mem::size_of,
};
use wtf_rs::unwrap_unchecked;
pub mod frame;
pub mod stack;

impl Runtime {
    pub(crate) fn perform_vm_call(
        &mut self,
        func: &JsVMFunction,
        env: JsValue,
        args_: &Arguments,
    ) -> Result<JsValue, JsValue> {
        let scope = unsafe { env.get_object().downcast_unchecked::<JsObject>() };
        let structure = Structure::new_indexed(self, Some(scope), false);

        let mut nscope = JsObject::new(self, structure, JsObject::get_class(), ObjectTag::Ordinary);

        let mut i = 0;
        for p in func.code.params.iter() {
            let _ = nscope
                .put(self, *p, args_.at(i), false)
                .unwrap_or_else(|_| unsafe { unreachable_unchecked() });
            i += 1;
        }

        if let Some(rest) = func.code.rest_param {
            let mut args_arr = JsArray::new(self, args_.size() as u32 - i as u32);
            let mut ix = 0;
            for _ in i..args_.size() {
                args_arr.put_indexed_slot(self, ix, args_.at(ix as _), &mut Slot::new(), false)?;
                ix += 1;
            }
            nscope.put(self, rest, JsValue::encode_object_value(args_arr), false)?;
        }
        for val in func.code.variables.iter() {
            nscope.define_own_property(
                self,
                *val,
                &*DataDescriptor::new(JsValue::encode_undefined_value(), W | C | E),
                false,
            )?;
        }

        let mut args = JsArguments::new(self, nscope.clone(), &func.code.params, args_.size() as _);

        for k in i..args_.size() {
            args.put(self, Symbol::Index(k as _), args_.at(k), false)?;
        }

        let _ = nscope.put(
            self,
            "arguments".intern(),
            JsValue::encode_object_value(args),
            false,
        )?;

        let _this = if func.code.strict && !args_.this.is_object() {
            JsValue::encode_undefined_value()
        } else {
            if args_.this.is_undefined() {
                JsValue::encode_object_value(self.global_object())
            } else {
                args_.this
            }
        };

        todo!()
    }
}

unsafe fn eval_internal(
    rt: &mut Runtime,
    code: GcPointer<CodeBlock>,
    ip: *mut u8,
    this: JsValue,
    ctor: bool,
    scope: GcPointer<JsObject>,
) -> Result<JsValue, JsValue> {
    let frame = rt.stack.new_frame();
    if frame.is_none() {
        let msg = JsString::new(rt, "stack overflow");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            rt, msg, None,
        )));
    }
    let frame = unwrap_unchecked(frame);
    (*frame).code_block = Some(code);
    (*frame).this = this;
    (*frame).env = JsValue::encode_object_value(scope);
    (*frame).ctor = ctor;
    (*frame).exit_on_return = true;
    (*frame).ip = ip;

    todo!()
}

pub unsafe fn eval(rt: &mut Runtime, frame: *mut CallFrame) -> Result<JsValue, JsValue> {
    let mut ip = (*frame).ip;
    let setup_frame = |rt: &mut Runtime,
                       frame: &mut CallFrame,
                       func: &JsVMFunction,
                       env: JsValue,
                       this: JsValue,
                       args_: GcPointer<ArrayStorage>|
     -> Result<(), JsValue> {
        let scope = unsafe { env.get_object().downcast_unchecked::<JsObject>() };
        let structure = Structure::new_indexed(rt, Some(scope), false);

        let mut nscope = JsObject::new(rt, structure, JsObject::get_class(), ObjectTag::Ordinary);

        let mut i = 0;
        for p in func.code.params.iter() {
            let _ = nscope
                .put(rt, *p, *args_.at(i), false)
                .unwrap_or_else(|_| unsafe { unreachable_unchecked() });
            i += 1;
        }

        if let Some(rest) = func.code.rest_param {
            let mut args_arr = JsArray::new(rt, args_.size() as u32 - i as u32);
            let mut ix = 0;
            for _ in i..args_.size() {
                args_arr.put_indexed_slot(rt, ix, *args_.at(ix as _), &mut Slot::new(), false)?;
                ix += 1;
            }
            nscope.put(rt, rest, JsValue::encode_object_value(args_arr), false)?;
        }
        for val in func.code.variables.iter() {
            nscope.define_own_property(
                rt,
                *val,
                &*DataDescriptor::new(JsValue::encode_undefined_value(), W | C | E),
                false,
            )?;
        }

        let mut args = JsArguments::new(rt, nscope.clone(), &func.code.params, args_.size() as _);

        for k in i..args_.size() {
            args.put(rt, Symbol::Index(k as _), *args_.at(k), false)?;
        }

        let _ = nscope.put(
            rt,
            "arguments".intern(),
            JsValue::encode_object_value(args),
            false,
        )?;
        let _this = if func.code.strict && !this.is_object() {
            JsValue::encode_undefined_value()
        } else {
            if this.is_undefined() {
                JsValue::encode_object_value(rt.global_object())
            } else {
                this
            }
        };
        frame.this = _this;
        frame.env = JsValue::encode_object_value(nscope);

        Ok(())
    };
    let mut frame: &'static mut CallFrame = &mut *frame;
    loop {
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);

        match opcode {
            Opcode::OP_NOP => {}
            Opcode::OP_JMP => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                ip = ip.offset(offset as isize);
            }
            Opcode::OP_POP => {
                frame.pop();
            }
            Opcode::OP_PUSH_INT => {
                let int = ip.cast::<i32>().read();
                ip = ip.add(4);
                frame.push(JsValue::encode_f64_value(int as f64));
            }
            Opcode::OP_PUSH_NAN => {
                frame.push(JsValue::encode_nan_value());
            }
            Opcode::OP_PUSH_NULL => {
                frame.push(JsValue::encode_null_value());
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
            Opcode::OP_ADD => {
                // let profile = &mut *ip.cast::<ArithProfile>();
                // ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                // profile.observe_lhs_and_rhs(lhs, rhs);

                if likely(lhs.is_number() && rhs.is_number()) {
                    let result = JsValue::encode_f64_value(lhs.get_number() + rhs.get_number());

                    frame.push(result);
                    continue;
                }

                let lhs = lhs.to_primitive(rt, JsHint::None)?;
                let rhs = rhs.to_primitive(rt, JsHint::None)?;

                if lhs.is_string() || rhs.is_string() {
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
                    frame.push(JsValue::encode_f64_value(lhs + rhs));
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
                    frame.push(JsValue::encode_f64_value(
                        lhs.get_number() - rhs.get_number(),
                    ));
                    continue;
                }
                // profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::encode_f64_value(lhs - rhs));
            }
            Opcode::OP_DIV => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_number() && rhs.is_number()) {
                    //    profile.lhs_saw_number();
                    //    profile.rhs_saw_number();
                    frame.push(JsValue::encode_f64_value(
                        lhs.get_number() / rhs.get_number(),
                    ));
                    continue;
                }
                //profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::encode_f64_value(lhs / rhs));
            }
            Opcode::OP_MUL => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();
                if likely(lhs.is_number() && rhs.is_number()) {
                    //  profile.lhs_saw_number();
                    //  profile.rhs_saw_number();
                    frame.push(JsValue::encode_f64_value(
                        lhs.get_number() * rhs.get_number(),
                    ));
                    continue;
                }
                //profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::encode_f64_value(lhs * rhs));
            }
            Opcode::OP_REM => {
                //let profile = &mut *ip.cast::<ArithProfile>();
                //ip = ip.add(size_of::<ArithProfile>());

                let lhs = frame.pop();
                let rhs = frame.pop();

                if likely(lhs.is_number() && rhs.is_number()) {
                    //  profile.lhs_saw_number();
                    //  profile.rhs_saw_number();
                    frame.push(JsValue::encode_f64_value(
                        lhs.get_number() % rhs.get_number(),
                    ));
                    continue;
                }
                // profile.observe_lhs_and_rhs(lhs, rhs);
                let lhs = lhs.to_number(rt)?;
                let rhs = rhs.to_number(rt)?;
                frame.push(JsValue::encode_f64_value(lhs % rhs));
            }
            Opcode::OP_SHL => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::encode_f64_value((left << (right & 0x1f)) as f64));
            }
            Opcode::OP_SHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::encode_f64_value((left >> (right & 0x1f)) as f64));
            }

            Opcode::OP_USHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_uint32(rt)?;
                let right = rhs.to_uint32(rt)?;
                frame.push(JsValue::encode_f64_value((left >> (right & 0x1f)) as f64));
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

            Opcode::OP_INSTANCEOF => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(rt, "'instanceof' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                let robj = rhs.get_jsobject();
                if unlikely(!robj.is_callable()) {
                    let msg = JsString::new(rt, "'instanceof' requires constructor");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }

                frame.push(JsValue::encode_bool_value(
                    robj.as_function().has_instance(robj, rt, lhs)?,
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
                    rhs.get_jsobject().has_property(rt, sym),
                ));
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
            Opcode::OP_GET_ENV => {
                let name = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let name = *unwrap_unchecked((*frame).code_block)
                    .names
                    .get_unchecked(name as usize);
                let result = 'search: loop {
                    let mut current = Some((*frame).env.get_jsobject());
                    while let Some(env) = current {
                        if (Env { record: env }).has_own_variable(rt, name) {
                            break 'search Some(env);
                        }
                        current = env.prototype().copied();
                    }
                    break 'search None;
                };

                match result {
                    Some(env) => frame.push(JsValue::encode_object_value(env)),
                    None => frame.push(JsValue::encode_undefined_value()),
                }
            }

            Opcode::OP_GET_VAR => {
                let name = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let name = *unwrap_unchecked((*frame).code_block)
                    .names
                    .get_unchecked(name as usize);
                let value = get_var(rt, name, frame, fdbk)?;
                frame.push(value);
            }
            Opcode::OP_SET_VAR => {
                let val = frame.pop();
                let name = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let name = *unwrap_unchecked((*frame).code_block)
                    .names
                    .get_unchecked(name as usize);
                set_var(rt, frame, name, fdbk, val)?;
            }
            Opcode::OP_GET_BY_ID => {
                let name = ip.cast::<u32>().read_unaligned();
                let name = *unwrap_unchecked(frame.code_block)
                    .names
                    .get_unchecked(name as usize);
                ip = ip.add(4);
                let fdbk = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let object = frame.pop();
                if likely(object.is_jsobject()) {
                    let obj = object.get_jsobject();
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

                    let mut slot = Slot::new();
                    if obj.get_property_slot(rt, name, &mut slot) && slot.is_load_cacheable() {
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
                    frame.push(slot.value());
                    continue;
                }

                fn get_by_id_slow(
                    rt: &mut Runtime,
                    name: Symbol,
                    val: JsValue,
                ) -> Result<JsValue, JsValue> {
                    let mut slot = Slot::new();
                    val.get_slot(rt, name, &mut slot)
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
            Opcode::OP_CALL => {
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let mut args = ArrayStorage::new(rt.heap(), argc);

                for _ in 0..argc {
                    let arg = frame.pop();
                    if unlikely(arg.is_object() && arg.get_object().is::<SpreadValue>()) {
                        let spread = arg.get_object().downcast_unchecked::<SpreadValue>();
                        for i in 0..spread.array.get(rt, "length".intern())?.get_number() as usize {
                            let real_arg = spread.array.get(rt, Symbol::Index(i as _))?;
                            args.push_back(rt.heap(), real_arg);
                        }
                    } else {
                        frame.push(arg);
                    }
                }
                let this = frame.pop();
                let func = frame.pop();
                if !func.is_callable() {
                    let msg = JsString::new(rt, "not a callable object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                let mut func_object = func.get_jsobject();
                let func = func_object.as_function_mut();
                if let FuncType::User(ref vm_function) = func.ty {
                    let new_frame = rt.stack.new_frame();
                    if new_frame.is_none() {
                        let msg = JsString::new(rt, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            rt, msg, None,
                        )));
                    }

                    let new_frame = unwrap_unchecked(new_frame);
                    (*new_frame).code_block = Some(vm_function.code);
                    (*new_frame).ctor = false;
                    (*new_frame).exit_on_return = false;
                    (*new_frame).ip = &vm_function.code.code[0] as *const u8 as *mut u8;
                    setup_frame(
                        rt,
                        &mut *new_frame,
                        vm_function,
                        JsValue::encode_object_value(vm_function.scope),
                        this,
                        args,
                    )?;
                    frame.ip = ip;
                    frame = &mut *new_frame;
                } else {
                    let mut args_ = Arguments::from_array_storage(rt, this, args);
                    let result = func.call(rt, &mut args_)?;
                    frame.push(result);
                }
            }
            _ => unreachable_unchecked(),
        }
    }
}
fn get_env(rt: &mut Runtime, frame: &mut CallFrame, name: Symbol) -> Option<GcPointer<JsObject>> {
    'search: loop {
        let mut current = Some((*frame).env.get_jsobject());
        while let Some(env) = current {
            if (Env { record: env }).has_own_variable(rt, name) {
                break 'search Some(env);
            }
            current = env.prototype().copied();
        }
        break 'search None;
    }
}

unsafe fn get_var(
    rt: &mut Runtime,
    name: Symbol,
    frame: &mut CallFrame,
    fdbk: u32,
) -> Result<JsValue, JsValue> {
    let env = get_env(rt, frame, name);
    let env = match env {
        Some(env) => env,
        None => rt.global_object(),
    };

    if let TypeFeedBack::PropertyCache { structure, offset } = unwrap_unchecked(frame.code_block)
        .feedback
        .get_unchecked(fdbk as usize)
    {
        if let Some(structure) = structure.upgrade() {
            if GcPointer::ptr_eq(&structure, &env.structure()) {
                return Ok(*env.direct(*offset as usize));
            }
        }
    }

    let mut slot = Slot::new();
    if likely(env.get_own_property_slot(rt, name, &mut slot)) {
        *unwrap_unchecked(frame.code_block)
            .feedback
            .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
            structure: rt.heap().make_weak(env.structure()),
            offset: slot.offset(),
        };

        let value = slot.value();
        return Ok(value);
    };
    let msg = JsString::new(
        rt,
        format!("Undeclared variable '{}'", rt.description(name)),
    );
    Err(JsValue::encode_object_value(JsReferenceError::new(
        rt, msg, None,
    )))
}

unsafe fn set_var(
    rt: &mut Runtime,
    frame: &mut CallFrame,
    name: Symbol,
    fdbk: u32,
    val: JsValue,
) -> Result<(), JsValue> {
    let env = get_env(rt, frame, name);
    let mut env = match env {
        Some(env) => env,
        None if !unwrap_unchecked(frame.code_block).strict => rt.global_object(),
        _ => {
            let msg = JsString::new(
                rt,
                format!("Unresolved reference '{}'", rt.description(name)),
            );
            return Err(JsValue::encode_object_value(JsReferenceError::new(
                rt, msg, None,
            )));
        }
    };
    if let TypeFeedBack::PropertyCache { structure, offset } = unwrap_unchecked(frame.code_block)
        .feedback
        .get_unchecked(fdbk as usize)
    {
        if let Some(structure) = structure.upgrade() {
            if GcPointer::ptr_eq(&structure, &env.structure()) {
                *env.direct_mut(*offset as usize) = val;
            }
        }
    }

    let mut slot = Slot::new();
    assert!(env.get_own_property_slot(rt, name, &mut slot));
    *unwrap_unchecked(frame.code_block)
        .feedback
        .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
        structure: rt.heap().make_weak(env.structure()),
        offset: slot.offset(),
    };
    *env.direct_mut(slot.offset() as usize) = val;
    Ok(())
}

/// Type used internally in JIT/interpreter to represent spread result.
pub struct SpreadValue {
    pub(crate) array: GcPointer<JsObject>,
}

impl SpreadValue {
    pub fn new(rt: &mut Runtime, value: JsValue) -> Result<GcPointer<Self>, JsValue> {
        unsafe {
            if value.is_jsobject() {
                if value.get_object().downcast_unchecked::<JsObject>().tag() == ObjectTag::Array {
                    return Ok(rt.heap().allocate(Self {
                        array: value.get_object().downcast_unchecked(),
                    }));
                }
            }

            let msg = JsString::new(rt, "cannot create spread from non-array value");
            Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )))
        }
    }
}

impl GcCell for SpreadValue {}
unsafe impl Trace for SpreadValue {
    fn trace(&self, visitor: &mut SlotVisitor) {
        self.array.trace(visitor);
    }
}
