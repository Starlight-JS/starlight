use self::{frame::CallFrame, stack::Stack};
use super::{
    arguments::*, array::*, array_storage::ArrayStorage, attributes::*, code_block::CodeBlock,
    error::JsTypeError, error::*, function::JsVMFunction, object::*, property_descriptor::*,
    slot::*, string::JsString, structure::*, symbol_table::*, value::*, Runtime,
};
use crate::bytecode::*;
use crate::{
    bytecode::opcodes::Opcode,
    heap::{
        cell::{GcCell, GcPointer, Trace},
        snapshot::deserializer::Deserializable,
        SlotVisitor,
    },
};
use std::{
    hint::unreachable_unchecked,
    intrinsics::{likely, unlikely},
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
            let mut ai = 0;
            for ix in i..args_.size() {
                args_arr.put_indexed_slot(
                    self,
                    ai as _,
                    args_.at(ix as _),
                    &mut Slot::new(),
                    false,
                )?;
                ai += 1;
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

        unsafe {
            eval_internal(
                self,
                func.code,
                &func.code.code[0] as *const u8 as *mut u8,
                _this,
                args_.ctor_call,
                nscope,
            )
        }
    }
}
#[inline(never)]
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

    loop {
        let result = eval(rt, frame);
        match result {
            Ok(value) => return Ok(value),
            Err(e) => {
                if let Some((env, ip)) = (*frame).try_stack.pop() {
                    (*frame).env = env;
                    (*frame).ip = ip;
                    (*frame).push(e);
                    continue;
                }
                return Err(e);
            }
        }
    }
}

pub unsafe fn eval(rt: &mut Runtime, frame: *mut CallFrame) -> Result<JsValue, JsValue> {
    let mut ip = (*frame).ip;
    let _setup_frame = |rt: &mut Runtime,
                        frame: &mut CallFrame,
                        func: &JsVMFunction,
                        env: JsValue,
                        this: JsValue,
                        args_: GcPointer<ArrayStorage>|
     -> Result<(), JsValue> {
        let scope = env.get_object().downcast_unchecked::<JsObject>();
        let structure = Structure::new_indexed(rt, Some(scope), false);

        let mut nscope = JsObject::new(rt, structure, JsObject::get_class(), ObjectTag::Ordinary);

        let mut i = 0;
        for p in func.code.params.iter() {
            let _ = nscope
                .put(rt, *p, *args_.at(i), false)
                .unwrap_or_else(|_| unreachable_unchecked());
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
    let stack = &mut rt.stack as *mut Stack;
    let stack = &mut *stack;
    loop {
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);

        stack.cursor = frame.sp;
        match opcode {
            Opcode::OP_NOP => {}

            Opcode::OP_JMP => {
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
            Opcode::OP_PUSH_LITERAL => {
                let ix = ip.cast::<u32>().read();
                ip = ip.add(4);
                let constant = unwrap_unchecked(frame.code_block).literals[ix as usize];
                //assert!(constant.is_js_string());
                frame.push(constant);
            }
            Opcode::OP_PUSH_THIS => {
                frame.push(frame.this);
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

                if lhs.is_js_string() || rhs.is_js_string() {
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
                //    println!("{} {}", lhs.get_number(), rhs.get_number());
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
            Opcode::OP_THROW => {
                let val = frame.pop();
                return Err(val);
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
                    frame.push(JsValue::encode_f64_value(-v1.get_number()));
                } else {
                    let n = v1.to_number(rt)?;
                    frame.push(JsValue::encode_f64_value(-n));
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
                    if rt.options.inline_caches
                        && obj.get_property_slot(rt, name, &mut slot)
                        && slot.is_load_cacheable()
                    {
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
                    if rt.options.inline_caches {
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
                    if rt.options.inline_caches && slot.is_put_cacheable() {
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
            Opcode::OP_CALL => {
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let mut args = ArrayStorage::new(rt.heap(), argc);
                let func = frame.pop();
                let this = frame.pop();
                for _ in 0..argc {
                    let arg = frame.pop();
                    if unlikely(arg.is_object() && arg.get_object().is::<SpreadValue>()) {
                        let spread = arg.get_object().downcast_unchecked::<SpreadValue>();
                        for i in 0..spread.array.get(rt, "length".intern())?.get_number() as usize {
                            let real_arg = spread.array.get(rt, Symbol::Index(i as _))?;
                            args.push_back(rt.heap(), real_arg);
                        }
                    } else {
                        args.push_back(rt.heap(), arg);
                    }
                }

                if !func.is_callable() {
                    let msg = JsString::new(rt, "not a callable object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                let mut func_object = func.get_jsobject();
                let func = func_object.as_function_mut();
                /*if let FuncType::User(ref vm_function) = func.ty {
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
                } else {*/
                /* let mut next = ip.cast::<Opcode>().read();
                match next {
                    Opcode::OP_POP_ENV => {
                        next = ip.add(1).cast::<Opcode>().read();
                    }
                    _ => (),
                }
                match next {
                    Opcode::OP_RET => {
                        if func.is_native() {
                            if !frame.prev.is_null() {
                                stack.pop_frame();
                                let mut args_ = Arguments::from_array_storage(rt, this, args);
                                let native = func.as_native();
                                return (native.func)(rt, &mut args_);
                            }
                        } else if func.is_vm() {
                            if !frame.prev.is_null() {
                                stack.pop_frame();

                                let vm_function = func.as_vm();
                                let new_frame = stack.new_frame().unwrap();

                                (*new_frame).code_block = Some(vm_function.code);
                                (*new_frame).ctor = false;
                                (*new_frame).exit_on_return = true;
                                (*new_frame).ip = &vm_function.code.code[0] as *const u8 as *mut u8;
                                setup_frame(
                                    rt,
                                    &mut *new_frame,
                                    vm_function,
                                    JsValue::encode_object_value(vm_function.scope),
                                    this,
                                    args,
                                )?;

                                frame = &mut *new_frame;
                                ip = (*new_frame).ip;
                            }
                        }
                    }
                    _ => (),
                }*/
                let mut args_ = Arguments::from_array_storage(rt, this, args);
                let result = func.call(rt, &mut args_)?;
                frame.push(result);
            }
            Opcode::OP_NEW => {
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let mut args = ArrayStorage::new(rt.heap(), argc);
                let func = frame.pop();
                let this = frame.pop();

                for _ in 0..argc {
                    let arg = frame.pop();
                    if unlikely(arg.is_object() && arg.get_object().is::<SpreadValue>()) {
                        let spread = arg.get_object().downcast_unchecked::<SpreadValue>();
                        for i in 0..spread.array.get(rt, "length".intern())?.get_number() as usize {
                            let real_arg = spread.array.get(rt, Symbol::Index(i as _))?;
                            args.push_back(rt.heap(), real_arg);
                        }
                    } else {
                        args.push_back(rt.heap(), arg);
                    }
                }

                if !func.is_callable() {
                    let msg = JsString::new(rt, "not a callable object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        rt, msg, None,
                    )));
                }
                let mut func_object = func.get_jsobject();
                let map = func_object.func_construct_map(rt)?;
                let func = func_object.as_function_mut();
                let mut args_ = Arguments::from_array_storage(rt, this, args);
                args_.ctor_call = true;
                let result = func.construct(rt, &mut args_, Some(map))?;
                frame.push(result);
            }
            Opcode::OP_PUSH_CATCH => {
                let offset = ip.cast::<i32>().read();
                ip = ip.add(4);
                let env = frame.env;

                frame.try_stack.push((env, ip.offset(offset as isize)));
            }
            Opcode::OP_POP_CATCH => {
                frame.try_stack.pop();
            }
            Opcode::OP_PUSH_ENV => {
                let map = Structure::new_indexed(rt, Some(frame.env.get_jsobject()), false);
                let env = JsObject::new(rt, map, JsObject::get_class(), ObjectTag::Ordinary);
                frame.env = JsValue::encode_object_value(env);
            }
            Opcode::OP_POP_ENV => {
                let env = frame.env.get_jsobject();
                frame.env = JsValue::encode_object_value(
                    env.prototype().copied().expect("no environments left"),
                );
            }
            Opcode::OP_LOGICAL_NOT => {
                let val = frame.pop();
                frame.push(JsValue::encode_bool_value(!val.to_boolean()));
            }
            Opcode::OP_NOT => {
                let v1 = frame.pop();
                if v1.is_number() {
                    let n = v1.get_number() as i32;
                    frame.push(JsValue::encode_f64_value((!n) as _));
                } else {
                    let n = v1.to_number(rt)? as i32;
                    frame.push(JsValue::encode_f64_value((!n) as _));
                }
            }
            Opcode::OP_POS => {
                let value = frame.pop();
                if value.is_number() {
                    frame.push(value);
                }
                let x = value.to_number(rt)?;
                frame.push(JsValue::encode_f64_value(x));
            }

            Opcode::OP_DECL_CONST => {
                let val = frame.pop();
                let name = ip.cast::<u32>().read();
                ip = ip.add(8);
                let name = unwrap_unchecked(frame.code_block).names[name as usize];
                Env {
                    record: frame.env.get_jsobject(),
                }
                .declare_variable(rt, name, val, false)?;
            }
            Opcode::OP_DECL_LET => {
                let val = frame.pop();
                let name = ip.cast::<u32>().read();
                ip = ip.add(8);
                let name = unwrap_unchecked(frame.code_block).names[name as usize];
                Env {
                    record: frame.env.get_jsobject(),
                }
                .declare_variable(rt, name, val, true)?;
            }
            Opcode::OP_DELETE_VAR => {
                let name = ip.cast::<u32>().read();
                ip = ip.add(4);
                let name = unwrap_unchecked(frame.code_block).names[name as usize];
                let env = get_env(rt, frame, name);

                match env {
                    Some(mut env) => {
                        frame.push(JsValue::encode_bool_value(env.delete(rt, name, false)?))
                    }
                    None => {
                        frame.push(JsValue::encode_bool_value(true));
                    }
                }
            }
            Opcode::OP_GET_FUNCTION => {
                //vm.space().defer_gc();
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let func = JsVMFunction::new(
                    rt,
                    unwrap_unchecked(frame.code_block).codes[ix as usize],
                    (*frame).env.get_jsobject(),
                );
                assert!(func.is_callable());

                frame.push(JsValue::encode_object_value(func));
                // vm.space().undefer_gc();
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
                if value.is_number() {
                    //  println!("ret {}", value.get_number());
                }
                /* if frame.exit_on_return {
                    return Ok(value);
                }
                let _ = rt.stack.pop_frame().unwrap();
                frame = &mut *rt.stack.current;
                ip = frame.ip;*/
                return Ok(value);
            }
            Opcode::OP_PUSH_UNDEF => {
                frame.push(JsValue::encode_undefined_value());
            }
            Opcode::OP_NEWARRAY => {
                let count = ip.cast::<u32>().read_unaligned();

                ip = ip.add(4);
                let mut arr = JsArray::new(rt, count);
                let mut index = 0;
                while index < count {
                    let value = frame.pop();
                    if unlikely(value.is_object() && value.get_object().is::<SpreadValue>()) {
                        let spread = value.get_object().downcast_unchecked::<SpreadValue>();
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
                frame.push(JsValue::encode_object_value(arr));
            }
            x => panic!("{:?}", x),
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
        if slot.is_load_cacheable() {
            *unwrap_unchecked(frame.code_block)
                .feedback
                .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                structure: rt.heap().make_weak(env.structure()),
                offset: slot.offset(),
            };
        }

        let value = slot.value();
        // println!("{}", value.is_callable());
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
    if GcPointer::ptr_eq(&env, &rt.global_object()) {
        env.put(rt, name, val, unwrap_unchecked(frame.code_block).strict)?;
        return Ok(());
    }
    assert!(env.get_own_property_slot(rt, name, &mut slot));
    let slot = Env { record: env }.set_variable(
        rt,
        name,
        val,
        unwrap_unchecked(frame.code_block).strict,
    )?;
    *unwrap_unchecked(frame.code_block)
        .feedback
        .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
        structure: rt.heap().make_weak(slot.0.structure()),
        offset: slot.1.offset(),
    };
    //*env.direct_mut(slot.1.offset() as usize) = val;
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

impl GcCell for SpreadValue {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    vtable_impl!();
}
unsafe impl Trace for SpreadValue {
    fn trace(&self, visitor: &mut SlotVisitor) {
        self.array.trace(visitor);
    }
}
