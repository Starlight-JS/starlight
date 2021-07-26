/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use self::{frame::CallFrame, stack::Stack};
use super::function::*;
use super::{
    arguments::*, array::*, code_block::CodeBlock, environment::*, error::JsTypeError, error::*,
    native_iterator::*, object::*, slot::*, string::JsString, symbol_table::*, value::*,
};
use crate::letroot;
use crate::vm::class::JsClass;
use crate::vm::context::Context;
use crate::{
    bytecode::opcodes::Opcode,
    gc::{
        cell::{GcCell, GcPointer, Trace},
        snapshot::deserializer::Deserializable,
    },
};
use crate::{bytecode::*, gc::cell::Tracer};
use profile::{ArithProfile, ByValProfile};
use std::intrinsics::{likely, unlikely};
use wtf_rs::unwrap_unchecked;
pub mod callframe;
pub mod frame;
pub mod stack;

impl GcPointer<Context> {
    pub(crate) fn perform_vm_call(
        mut self,
        func: &JsVMFunction,
        env: JsValue,
        args_: &Arguments,
        callee: JsValue,
    ) -> Result<JsValue, JsValue> {
        let mut scope = unsafe { env.get_object().downcast::<Environment>().unwrap() };

        let mut nscope = Environment::new(
            self,
            func.code.param_count
                + func.code.var_count
                + func.code.rest_at.map(|_| 1).unwrap_or(0)
                + if func.code.use_arguments { 1 } else { 0 },
        );
        nscope.parent = Some(scope);
        let mut i = 0;
        for _ in 0..func.code.param_count {
            nscope.as_slice_mut()[i as usize].value = args_.at(i);
            i += 1;
        }

        if let Some(rest) = func.code.rest_at {
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
            nscope.as_slice_mut()[rest as usize].value = JsValue::new(args_arr);
        }

        if func.code.use_arguments {
            let p = {
                let mut p = vec![];
                for i in 0..func.code.param_count {
                    p.push(Symbol::Index(i));
                }
                p
            };
            let mut args = JsArguments::new(self, nscope, &p, args_.size() as _, args_.values);

            for k in i..args_.size() {
                args.put(self, Symbol::Index(k as _), args_.at(k), false)?;
            }

            nscope.as_slice_mut()[func.code.args_at as usize].value = JsValue::new(args);
        }
        let _this = if func.code.strict && !args_.this.is_object() {
            JsValue::encode_undefined_value()
        } else if args_.this.is_undefined() {
            JsValue::encode_object_value(self.global_object())
        } else {
            args_.this
        };

        unsafe {
            eval_internal(
                self,
                func.code,
                &func.code.code[0] as *const u8 as *mut u8,
                _this,
                args_.ctor_call,
                nscope,
                callee,
            )
        }
    }

    pub(crate) fn setup_for_vm_call(
        mut self,
        func: &JsVMFunction,
        env: JsValue,
        args_: &Arguments,
    ) -> Result<(JsValue, GcPointer<Environment>), JsValue> {
        let stack = self.shadowstack();
        letroot!(scope = stack, unsafe {
            env.get_object().downcast::<Environment>().unwrap()
        });

        letroot!(
            nscope = stack,
            Environment::new(
                self,
                func.code.param_count
                    + func.code.var_count
                    + func.code.rest_at.map(|_| 1).unwrap_or(0)
                    + if func.code.use_arguments { 1 } else { 0 }
            )
        );
        nscope.parent = Some(*scope);
        let mut i = 0;
        for _ in 0..func.code.param_count {
            /*let _ = nscope
                .put(self, *p, args_.at(i), false)
                .unwrap_or_else(|_| unsafe { unreachable_unchecked() });
            */
            nscope.as_slice_mut()[i as usize].value = args_.at(i);
            i += 1;
        }

        if let Some(rest) = func.code.rest_at {
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
            nscope.as_slice_mut()[rest as usize].value = JsValue::new(args_arr);
            //  nscope.put(self, rest, JsValue::encode_object_value(args_arr), false)?;
        }

        /*for j in 0..func.code.var_count {
            vscope.as_slice_mut()[j as usize + i as usize].0 = JsValue::encode_undefined_value();

            //   vscope.put(self, *val, JsValue::encode_undefined_value(), false)?;
        }*/
        if func.code.use_arguments {
            let p = {
                let mut p = vec![];
                for i in 0..func.code.param_count {
                    p.push(Symbol::Index(i));
                }
                p
            };
            let mut args = JsArguments::new(self, *nscope, &p, args_.size() as _, args_.values);

            for k in i..args_.size() {
                args.put(self, Symbol::Index(k as _), args_.at(k), false)?;
            }

            nscope.as_slice_mut()[func.code.args_at as usize].value = JsValue::new(args);
        }
        let _this = if func.code.strict && !args_.this.is_object() {
            JsValue::encode_undefined_value()
        } else if args_.this.is_undefined() {
            JsValue::encode_object_value(self.global_object())
        } else {
            args_.this
        };

        Ok((_this, *nscope))
    }
}

#[inline(never)]
unsafe fn eval_internal(
    mut ctx: GcPointer<Context>,
    code: GcPointer<CodeBlock>,
    ip: *mut u8,
    this: JsValue,
    ctor: bool,
    scope: GcPointer<Environment>,
    callee: JsValue,
) -> Result<JsValue, JsValue> {
    let frame = ctx.stack.new_frame(0, callee, scope);
    if frame.is_none() {
        let msg = JsString::new(ctx, "stack overflow");
        return Err(JsValue::encode_object_value(JsRangeError::new(
            ctx, msg, None,
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
        let result = eval(ctx, frame);
        match result {
            Ok(value) => return Ok(value),
            Err(e) => {
                ctx.stacktrace = ctx.stacktrace();

                if let Some(unwind_frame) = ctx.unwind() {
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

pub unsafe fn eval(mut ctx: GcPointer<Context>, frame: *mut CallFrame) -> Result<JsValue, JsValue> {
    ctx.heap().collect_if_necessary();
    let mut ip = (*frame).ip;

    let mut frame: &'static mut CallFrame = &mut *frame;
    let stack = &mut ctx.stack as *mut Stack;
    let stack = &mut *stack;
    let gcstack = ctx.shadowstack();
    loop {
        let opcode = ip.cast::<Opcode>().read_unaligned();
        ip = ip.add(1);
        #[cfg(feature = "perf")]
        {
            ctx.perf.get_perf(opcode as u8);
        }
        /*println!(
            "exec block({:p}): {}: {:?} (sp {})",
            unwrap_unchecked(frame.code_block),
            ip.sub(1).offset_from(&frame.code_block.unwrap().code[0]),
            opcode,
            frame.sp.offset_from(frame.limit)
        );
        let mut scan = frame.limit;
        print!("Stack: [");
        while scan < frame.sp {
            print!(
                " ({}) ",
                scan.read()
                    .to_string(ctx)
                    .unwrap_or_else(|_| "<unknown>".to_string())
            );
            scan = scan.add(1);
        }
        print!("]\n");*/
        stack.cursor = frame.sp;
        match opcode {
            Opcode::OP_GE0GL => {
                let index = ip.cast::<u32>().read_unaligned();
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
                ip = ip.add(4);
                let mut env = frame.env;
                debug_assert!(index < env.as_slice_mut().len() as u32);
                let val = frame.pop();
                if unlikely(!env.as_slice_mut()[index as usize].mutable) {
                    return Err(JsValue::new(
                        ctx.new_type_error("Cannot assign to immutable variable".to_string()),
                    ));
                }

                env.as_slice_mut().get_unchecked_mut(index as usize).value = val;
            }
            Opcode::OP_GET_LOCAL => {
                let index = ip.cast::<u32>().read_unaligned();
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
                ip = ip.add(4);
                let mut env = frame.pop().get_object().downcast::<Environment>().unwrap();
                debug_assert!(index < env.as_slice_mut().len() as u32);
                let val = frame.pop();
                if unlikely(!env.as_slice_mut()[index as usize].mutable) {
                    return Err(JsValue::new(
                        ctx.new_type_error("Cannot assign to immutable variable".to_string()),
                    ));
                }

                env.as_slice_mut().get_unchecked_mut(index as usize).value = val;
            }
            Opcode::OP_GET_ENV => {
                let mut depth = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let mut env = frame.env;

                while depth != 0 {
                    env = unwrap_unchecked(env.parent);
                    depth -= 1;
                }

                frame.push(JsValue::new(env));
            }

            Opcode::OP_JMP => {
                ctx.heap().collect_if_necessary();
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
                //assectx!(constant.is_jsstring());
                frame.push(constant);
            }
            Opcode::OP_PUSH_THIS => {
                frame.push(frame.this);
            }
            Opcode::OP_PUSH_INT => {
                let int = ip.cast::<i32>().read();

                ip = ip.add(4);
                frame.push(JsValue::encode_int32(int));
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
                let prev = ctx.stack.pop_frame().unwrap();
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
                    ctx: GcPointer<Context>,
                    frame: &mut CallFrame,
                    lhs: JsValue,
                    rhs: JsValue,
                ) -> Result<(), JsValue> {
                    let lhs = lhs.to_primitive(ctx, JsHint::None)?;
                    let rhs = rhs.to_primitive(ctx, JsHint::None)?;

                    if lhs.is_jsstring() || rhs.is_jsstring() {
                        #[inline(never)]
                        fn concat(
                            ctx: GcPointer<Context>,
                            lhs: JsValue,
                            rhs: JsValue,
                        ) -> Result<JsValue, JsValue> {
                            let lhs = lhs.to_string(ctx)?;
                            let rhs = rhs.to_string(ctx)?;
                            let string = format!("{}{}", lhs, rhs);
                            Ok(JsValue::encode_object_value(JsString::new(ctx, string)))
                        }

                        let result = concat(ctx, lhs, rhs)?;
                        frame.push(result);
                    } else {
                        let lhs = lhs.to_number(ctx)?;
                        let rhs = rhs.to_number(ctx)?;
                        frame.push(JsValue::new(lhs + rhs));
                    }
                    Ok(())
                }
                add_slowpath(ctx, frame, lhs, rhs)?;
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
                let lhs = lhs.to_number(ctx)?;
                let rhs = rhs.to_number(ctx)?;
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

                let lhs = lhs.to_number(ctx)?;
                let rhs = rhs.to_number(ctx)?;
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
                let lhs = lhs.to_number(ctx)?;
                let rhs = rhs.to_number(ctx)?;
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
                let lhs = lhs.to_number(ctx)?;
                let rhs = rhs.to_number(ctx)?;
                frame.push(JsValue::new(lhs % rhs));
            }
            Opcode::OP_SHL => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(ctx)?;
                let right = rhs.to_uint32(ctx)?;
                frame.push(JsValue::new((left << (right & 0x1f)) as f64));
            }
            Opcode::OP_SHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_int32(ctx)?;
                let right = rhs.to_uint32(ctx)?;
                frame.push(JsValue::new((left >> (right & 0x1f)) as f64));
            }

            Opcode::OP_USHR => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                let left = lhs.to_uint32(ctx)?;
                let right = rhs.to_uint32(ctx)?;
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
                    lhs.compare(rhs, true, ctx)? == CMP_TRUE,
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
                    rhs.compare(lhs, false, ctx)? == CMP_FALSE,
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
                    rhs.compare(lhs, false, ctx)? == CMP_TRUE,
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
                    lhs.compare(rhs, true, ctx)? == CMP_FALSE,
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
                    if let TypeFeedBack::PropertyCache {
                        structure,
                        offset,
                        mode,
                    } = unwrap_unchecked(frame.code_block)
                        .feedback
                        .get_unchecked(fdbk as usize)
                    {
                        match mode {
                            &GetByIdMode::Default => {
                                if GcPointer::ptr_eq(structure, &obj.structure()) {
                                    frame.push(*obj.direct(*offset as _));

                                    continue;
                                }
                            }
                            GetByIdMode::ProtoLoad(base) => {
                                if false && GcPointer::ptr_eq(structure, &obj.structure()) {
                                    frame.push(*base.direct(*offset as _));

                                    continue;
                                }
                            }
                            &GetByIdMode::ArrayLength => {
                                if obj.is_class(JsArray::class()) {
                                    frame.push(JsValue::new(obj.indexed.length()));
                                    continue;
                                }
                            }
                        }
                    }

                    #[inline(never)]
                    #[cold]
                    unsafe fn slow_get_by_id(
                        ctx: GcPointer<Context>,
                        frame: &mut CallFrame,
                        obj: &mut GcPointer<JsObject>,
                        name: Symbol,
                        fdbk: u32,
                        is_try: bool,
                    ) -> Result<(), JsValue> {
                        let mut slot = Slot::new();
                        if name == length_id() && obj.is_class(JsArray::class()) {
                            *unwrap_unchecked(frame.code_block)
                                .feedback
                                .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                                structure: obj.structure(),
                                mode: GetByIdMode::ArrayLength,
                                offset: u32::MAX,
                            };
                            frame.push(JsValue::new(obj.indexed.length()));
                            return Ok(());
                        }
                        let found = obj.get_property_slot(ctx, name, &mut slot);
                        #[cfg(not(feature = "no-inline-caching"))]
                        if slot.is_load_cacheable() {
                            let (structure, mode) = match slot.base() {
                                Some(object) => {
                                    if let Some(proto) = obj.prototype() {
                                        if GcPointer::ptr_eq(proto, object) {
                                            (
                                                obj.structure(),
                                                GetByIdMode::ProtoLoad(object.downcast_unchecked()),
                                            )
                                        } else {
                                            (
                                                slot.base()
                                                    .unwrap()
                                                    .downcast_unchecked::<JsObject>()
                                                    .structure(),
                                                GetByIdMode::Default,
                                            )
                                        }
                                    } else {
                                        (
                                            slot.base()
                                                .unwrap()
                                                .downcast_unchecked::<JsObject>()
                                                .structure(),
                                            GetByIdMode::Default,
                                        )
                                    }
                                }

                                None => unreachable!(),
                            };

                            *unwrap_unchecked(frame.code_block)
                                .feedback
                                .get_unchecked_mut(fdbk as usize) = TypeFeedBack::PropertyCache {
                                structure,
                                mode,
                                offset: slot.offset(),
                            }
                        }
                        if found {
                            frame.push(slot.get(ctx, JsValue::new(*obj))?);
                        } else {
                            if unlikely(is_try) {
                                let desc = ctx.description(name);
                                return Err(JsValue::new(ctx.new_reference_error(format!(
                                    "Property '{}' not found",
                                    desc
                                ))));
                            }
                            frame.push(JsValue::encode_undefined_value());
                        }
                        Ok(())
                    }
                    slow_get_by_id(
                        ctx,
                        frame,
                        &mut obj,
                        name,
                        fdbk,
                        opcode == Opcode::OP_TRY_GET_BY_ID,
                    )?;
                    continue;
                }
                frame.push(get_by_id_slow(ctx, name, object)?)
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

                        put_by_id_slow(ctx, frame, &mut obj, name, value, fdbk)?;
                        break 'exit;
                    }
                    continue;
                }
            }

            Opcode::OP_CALL | Opcode::OP_TAILCALL => {
                ctx.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);

                let args_start = frame.sp.sub(argc as _);

                frame.sp = args_start;
                let mut func = frame.pop();
                let mut this = frame.pop();
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);
                if unlikely(!func.is_callable()) {
                    let msg = JsString::new(ctx, "not a callable object".to_string());
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
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
                    let (this, scope) = ctx.setup_for_vm_call(vm_fn, scope, &args_)?;
                    let mut exit = false;
                    if !frame.exit_on_return
                        && (opcode == Opcode::OP_TAILCALL
                            || (ip.cast::<Opcode>().read() == Opcode::OP_POP
                                && ip.add(1).cast::<Opcode>().read() == Opcode::OP_RET))
                    {
                        // ctx.stack.pop_frame().unwrap();
                        exit = ctx.stack.pop_frame().unwrap().exit_on_return;
                    }
                    let cframe = ctx.stack.new_frame(0, JsValue::new(*funcc), scope);
                    if unlikely(cframe.is_none()) {
                        let msg = JsString::new(ctx, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            ctx, msg, None,
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
                    let result = func.call(ctx, &mut args_, JsValue::new(*funcc))?;
                    frame.push(result);
                }
            }
            Opcode::OP_NEW | Opcode::OP_TAILNEW => {
                ctx.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);

                let args_start = frame.sp.sub(argc as _);
                frame.sp = args_start;
                let mut func = frame.pop();
                let mut _this = frame.pop();
                let mut args = std::slice::from_raw_parts_mut(args_start, argc as _);

                if unlikely(!func.is_callable()) {
                    let msg = JsString::new(ctx, "not a callable constructor object ".to_string());
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }

                letroot!(func_object = gcstack, func.get_jsobject());
                letroot!(funcc = gcstack, func.get_jsobject());
                let map = func_object.func_construct_map(ctx)?;
                let func = func_object.as_function_mut();
                let object = JsObject::new(ctx, &map, JsObject::class(), ObjectTag::Ordinary);
                letroot!(
                    args_ = gcstack,
                    Arguments::new(JsValue::new(object), &mut args)
                );

                args_.ctor_call = true;
                frame.ip = ip;

                if func.is_vm() {
                    let vm_fn = func.as_vm_mut();
                    let scope = JsValue::new(vm_fn.scope);
                    let (this, scope) = ctx.setup_for_vm_call(vm_fn, scope, &args_)?;
                    let mut exit = false;
                    if !frame.exit_on_return && (opcode == Opcode::OP_TAILNEW) {
                        // stack.pop_frame().unwrap();
                        exit = stack.pop_frame().unwrap().exit_on_return;
                    }
                    let cframe = ctx.stack.new_frame(0, JsValue::new(*funcc), scope);
                    if unlikely(cframe.is_none()) {
                        let msg = JsString::new(ctx, "stack overflow");
                        return Err(JsValue::encode_object_value(JsRangeError::new(
                            ctx, msg, None,
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
                    let result = func.call(ctx, &mut args_, JsValue::new(*funcc))?;

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
                    let n = v1.to_number(ctx)?;
                    frame.push(JsValue::new(-n));
                }
            }

            Opcode::OP_EQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();

                frame.push(JsValue::encode_bool_value(lhs.abstract_equal(rhs, ctx)?));
            }
            Opcode::OP_STRICTEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(lhs.strict_equal(rhs)));
            }
            Opcode::OP_NEQ => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                frame.push(JsValue::encode_bool_value(!lhs.abstract_equal(rhs, ctx)?));
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
                    if likely(object.indexed.dense())
                        && likely(index < object.indexed.vector.size())
                    {
                        *object.indexed.vector.at_mut(index) = value;
                        continue;
                    }
                }
                let key = key.to_symbol(ctx)?;

                if likely(object.is_jsobject()) {
                    let mut obj = object.get_jsobject();
                    obj.put(ctx, key, value, unwrap_unchecked(frame.code_block).strict)?;
                } else {
                    #[inline(never)]
                    unsafe fn slow(
                        ctx: GcPointer<Context>,
                        object: JsValue,
                        key: Symbol,
                        value: JsValue,
                        strict: bool,
                    ) -> Result<JsValue, JsValue> {
                        object.to_object(ctx)?.put(ctx, key, value, strict)?;
                        Ok(JsValue::encode_undefined_value())
                    }

                    slow(
                        ctx,
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
                        && likely(index < object.indexed.vector.size() as usize)
                        && likely(!object.indexed.vector.at(index as _).is_empty())
                    {
                        if opcode == Opcode::OP_GET_BY_VAL_PUSH_OBJ {
                            frame.push(JsValue::new(object));
                        }
                        frame.push(*object.indexed.vector.at(index as _));

                        continue;
                    }
                }
                let key = key.to_symbol(ctx)?;
                let mut slot = Slot::new();
                let _ = object.get_slot(ctx, key, &mut slot)?;

                let value = slot.get(ctx, JsValue::new(object))?;

                if opcode == Opcode::OP_GET_BY_VAL_PUSH_OBJ {
                    frame.push(JsValue::new(object));
                }
                frame.push(value);
            }
            Opcode::OP_INSTANCEOF => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(ctx, "'instanceof' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }

                letroot!(robj = gcstack, rhs.get_jsobject());
                letroot!(robj2 = gcstack, *robj);
                if unlikely(!robj.is_callable()) {
                    let msg = JsString::new(ctx, "'instanceof' requires constructor");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }

                frame.push(JsValue::encode_bool_value(
                    robj.as_function().has_instance(&mut robj2, ctx, lhs)?,
                ));
            }
            Opcode::OP_IN => {
                let lhs = frame.pop();
                let rhs = frame.pop();
                if unlikely(!rhs.is_jsobject()) {
                    let msg = JsString::new(ctx, "'in' requires object");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }
                let sym = lhs.to_symbol(ctx)?;
                frame.push(JsValue::encode_bool_value(
                    rhs.get_jsobject().has_own_property(ctx, sym),
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
                    NativeIterator::new(ctx, enumerable.get_object())
                } else {
                    let obj = enumerable.to_object(ctx)?;
                    NativeIterator::new(ctx, obj.as_dyn())
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
                    let desc = ctx.description(sym);
                    frame.push(JsValue::new(JsString::new(ctx, desc)));
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
                let global = ctx.global_object();
                frame.push(JsValue::encode_object_value(global));
            }

            Opcode::OP_NEWOBJECT => {
                let obj = JsObject::new_empty(ctx);
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
                    let n = v1.to_number(ctx)? as i32;
                    frame.push(JsValue::new((!n) as i32));
                }
            }
            Opcode::OP_POS => {
                let value = frame.pop();
                if value.is_number() {
                    frame.push(value);
                }
                let x = value.to_number(ctx)?;
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
                object.check_object_coercible(ctx)?;
                letroot!(object = gcstack, object.to_object(ctx)?);
                frame.push(JsValue::new(object.delete(
                    ctx,
                    name,
                    unwrap_unchecked(frame.code_block).strict,
                )?));
            }
            Opcode::OP_DELETE_BY_VAL => {
                let object = frame.pop();
                let name = frame.pop().to_symbol(ctx)?;
                object.check_object_coercible(ctx)?;
                letroot!(object = gcstack, object.to_object(ctx)?);
                frame.push(JsValue::new(object.delete(
                    ctx,
                    name,
                    unwrap_unchecked(frame.code_block).strict,
                )?));
            }
            Opcode::OP_AND => {
                let lhs = frame.pop().to_int32(ctx)?;
                let rhs = frame.pop().to_int32(ctx)?;
                frame.push(JsValue::new(lhs & rhs));
            }
            Opcode::OP_OR => {
                let lhs = frame.pop().to_int32(ctx)?;
                let rhs = frame.pop().to_int32(ctx)?;
                frame.push(JsValue::new(lhs | rhs));
            }
            Opcode::OP_XOR => {
                let lhs = frame.pop().to_int32(ctx)?;
                let rhs = frame.pop().to_int32(ctx)?;
                frame.push(JsValue::new(lhs ^ rhs));
            }
            Opcode::OP_GET_FUNCTION => {
                //vm.space().defer_gc();
                let ix = ip.cast::<u32>().read_unaligned();
                ip = ip.add(4);
                let code = unwrap_unchecked(frame.code_block).codes[ix as usize];
                let func = if likely(!(code.is_async || code.is_generator)) {
                    JsVMFunction::new(ctx, code, frame.env)
                } else {
                    let func = JsVMFunction::new(ctx, code, frame.env);

                    JsGeneratorFunction::new(ctx, func)
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
                letroot!(arr = gcstack, JsArray::new(ctx, count));
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
                            arr.put(ctx, Symbol::Index(index), real_arg, false)?;
                            index += 1;
                        }
                    } else {
                        arr.put(ctx, Symbol::Index(index), value, false)?;
                        index += 1;
                    }
                    did_put += 1;
                }
                frame.push(JsValue::encode_object_value(*arr));
            }

            Opcode::OP_CALL_BUILTIN => {
                ctx.heap().collect_if_necessary();
                let argc = ip.cast::<u32>().read();
                ip = ip.add(4);
                let builtin_id = ip.cast::<u32>().read();
                ip = ip.add(4);
                let effect = ip.cast::<u32>().read();
                ip = ip.add(4);
                super::builtins::BUILTIN_FUNCS[builtin_id as usize](
                    ctx,
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
                    User code can't get access to this value, if it does this should be repoctxed.
                */
                let value = frame.pop();
                let spread = SpreadValue::new(ctx, value)?;
                frame.push(JsValue::encode_object_value(spread));
            }
            Opcode::OP_TYPEOF => {
                let val = frame.pop();
                let str = JsString::new(ctx, val.type_of());
                frame.push(JsValue::new(str));
            }
            Opcode::OP_TO_INTEGER_OR_INFINITY => {
                let number = frame.pop().to_number(ctx)?;
                if number.is_nan() || number == 0.0 {
                    frame.push(JsValue::encode_int32(0));
                } else {
                    frame.push(JsValue::new(number.trunc()));
                }
            }
            Opcode::OP_TO_LENGTH => {
                let n = frame.pop().to_length(ctx)?;
                frame.push(JsValue::new(n));
            }
            Opcode::OP_TO_OBJECT => {
                let target = frame.pop();
                let message = frame.pop();
                if unlikely(target.is_null() || target.is_undefined()) {
                    let msg = message.to_string(ctx)?;
                    return Err(JsValue::new(ctx.new_type_error(msg)));
                }
                frame.push(JsValue::new(target.to_object(ctx)?));
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
            Opcode::OP_IS_OBJECT => {
                let val = frame.pop();
                frame.push(JsValue::new(val.is_jsobject()));
            }
            Opcode::OP_NOP => todo!(),
            Opcode::OP_LOOPHINT => todo!(),
            Opcode::OP_PUSH_ENV => todo!(),
            Opcode::OP_POP_ENV => todo!(),
            Opcode::OP_SET_ENV => todo!(),
            Opcode::OP_SET_GLOBAL => todo!(),
            Opcode::OP_GET_GLOBAL => todo!(),
            Opcode::OP_DELETE_VAR => todo!(),
            Opcode::OP_FOROF_SETUP => todo!(),
            Opcode::OP_FOROF_ENUMERATE => todo!(),
            Opcode::OP_FOROF_LEAVE => todo!(),
            Opcode::OP_GE0DL => todo!(),
            Opcode::OP_GE0DC => todo!(),
            Opcode::OP_NEWGENERATOR => todo!(),
        }
    }
}

/// Type used internally in JIT/interpreter to represent spread result.
pub struct SpreadValue {
    pub(crate) array: Vec<JsValue>,
}

impl SpreadValue {
    pub fn new(mut ctx: GcPointer<Context>, value: JsValue) -> Result<GcPointer<Self>, JsValue> {
        let mut builtin = ctx.global_data.spread_builtin.unwrap();
        let mut slice = [value];
        let mut args = Arguments::new(JsValue::encode_undefined_value(), &mut slice);
        builtin
            .as_function_mut()
            .call(ctx, &mut args, JsValue::encode_undefined_value())
            .and_then(|x| {
                assert!(x.is_jsobject() && x.get_jsobject().is_class(JsArray::class()));
                let mut array = TypedJsObject::<JsArray>::new(x);
                let mut vec = vec![];
                for i in 0..crate::jsrt::get_length(ctx, &mut array.object())? {
                    vec.push(array.get(ctx, Symbol::Index(i))?);
                }
                Ok(ctx.heap().allocate(Self { array: vec }))
            })
    }
}

impl GcCell for SpreadValue {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}
unsafe impl Trace for SpreadValue {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.array.trace(visitor);
    }
}

pub fn get_by_id_slow(
    ctx: GcPointer<Context>,
    name: Symbol,
    val: JsValue,
) -> Result<JsValue, JsValue> {
    let mut slot = Slot::new();
    val.get_slot(ctx, name, &mut slot)
}

pub(crate) unsafe fn put_by_id_slow(
    ctx: GcPointer<Context>,
    frame: &mut CallFrame,
    obj: &mut GcPointer<JsObject>,
    name: Symbol,
    value: JsValue,
    fdbk: u32,
) -> Result<(), JsValue> {
    let mut slot = Slot::new();
    let _old_structure = obj.structure();
    obj.put_slot(
        ctx,
        name,
        value,
        &mut slot,
        unwrap_unchecked(frame.code_block).strict,
    )?;
    #[cfg(not(feature = "no-inline-caching"))]
    if slot.is_put_cacheable() && slot.base.is_some() {
        let mut base_cell = *obj;
        let mut new_structure = base_cell.structure();
        let mut m_old_structure;
        let mut m_offset;
        let mut m_new_structure = None;
        let mut m_new_chain = None;

        if GcPointer::ptr_eq(&base_cell, &slot.base.unwrap()) {
            if slot.put_result_type() == PutResultType::New {
                // TODO: This kind of IC does not work yet so it is not enabled to not waste time on
                // trying to setup new IC entry.
                return Ok(());
                /*if !new_structure.is_unique()
                    && new_structure
                        .previous
                        .map(|x| new_structure.storage_capacity() == x.storage_capacity())
                        .unwrap_or(false)
                {
                    assectx!(GcPointer::ptr_eq(
                        &new_structure.previous.unwrap(),
                        &old_structure
                    ));

                    {
                        let (result, saw_poly_proto) =
                            crate::vm::operations::normalize_prototype_chain(ctx, &base_cell);

                        if result != usize::MAX && !saw_poly_proto {
                            m_old_structure = Some(old_structure);
                            m_offset = slot.offset();
                            m_new_structure = Some(new_structure);
                            m_new_chain = Some(new_structure.prototype_chain(ctx, base_cell));
                        }
                    }
                }*/
            } else {
                m_old_structure = Some(new_structure);
                m_offset = slot.offset();
            }

            unwrap_unchecked(frame.code_block).feedback[fdbk as usize] =
                TypeFeedBack::PutByIdFeedBack {
                    new_structure: m_new_structure,
                    old_structure: m_old_structure,
                    offset: m_offset,
                    structure_chain: m_new_chain,
                };
            debug_assert!(!matches!(
                unwrap_unchecked(frame.code_block).feedback[fdbk as usize],
                TypeFeedBack::None
            ));
        }
    }

    Ok(())
}
