use crate::bytecode::profile::*;
use crate::{
    bytecode::opcodes::Opcode,
    heap::{
        cell::{GcCell, GcPointer, Trace},
        SlotVisitor,
    },
};
use wtf_rs::unwrap_unchecked;

use self::frame::CallFrame;

use super::{
    arguments::*, array::*, attributes::*, code_block::CodeBlock, error::JsTypeError, error::*,
    function::JsVMFunction, object::*, property_descriptor::*, slot::*, string::JsString,
    structure::*, symbol_table::*, value::*, Runtime,
};
use std::{hint::unreachable_unchecked, intrinsics::likely, mem::size_of};
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
            _ => unreachable_unchecked(),
        }
    }
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
