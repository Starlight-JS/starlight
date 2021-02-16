use crate::{
    bytecode::{opcodes::Op, TypeFeedBack},
    runtime::{
        arguments::Arguments,
        env::Env,
        error::{JsError, JsTypeError},
        function::JsVMFunction,
        js_arguments::JsArguments,
        object::{JsHint, JsObject, ObjectTag},
        slot::Slot,
        string::JsString,
        structure::Structure,
        symbol::Symbol,
        value::{CMP_FALSE, CMP_TRUE},
    },
};
use frame::FrameBase;
use std::ptr::null_mut;

use crate::{bytecode::ByteCode, heap::cell::Gc, runtime::value::JsValue, vm::VirtualMachine};

pub mod frame;
const LOG: bool = false;
unsafe fn eval_bcode(vm: &mut VirtualMachine, frame: *mut FrameBase) -> Result<JsValue, JsValue> {
    //let mut pc = (*frame).code;
    if LOG {
        println!("enter frame {:p}", frame);
    }
    let ctx = vm.space().persistent_context();
    let bcode = (*frame).bcode.unwrap();
    let mut pc = (*frame).code;
    loop {
        let op = std::mem::transmute::<_, Op>(pc.cast::<u8>().read_unaligned());
        pc = pc.add(1);
        if LOG {
            println!("{:?}", op);
        }
        (*frame).code = pc;
        match op {
            Op::OP_PLACEHOLDER => unreachable!("Placeholder op emitted by compiler"),
            Op::OP_DROP => {
                vm.pop();
            }
            Op::OP_DUP => {
                let v1 = vm.upop();
                vm.upush(v1);
                vm.upush(v1);
            }
            Op::OP_2DUP => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                vm.upush(v1);
                vm.upush(v2);
                vm.upush(v1);
                vm.upush(v2);
            }

            Op::OP_SWAP => {
                let v1 = vm.upop();
                let v2 = vm.upop();
                vm.push(v1);
                vm.push(v2);
            }
            Op::OP_SWAP_DROP => {
                let v1 = vm.upop();
                vm.pop();
                vm.upush(v1);
            }
            Op::OP_PUSH_UNDEFINED => {
                vm.upush(JsValue::undefined());
            }
            Op::OP_PUSH_NULL => {
                vm.upush(JsValue::null());
            }
            Op::OP_PUSH_THIS => {
                let this = vm.get_this();
                vm.upush(this);
            }
            Op::OP_PUSH_TRUE => {
                vm.upush(JsValue::new(true));
            }
            Op::OP_PUSH_FALSE => vm.upush(JsValue::new(false)),
            Op::OP_PUSH_ZERO => {
                vm.upush(JsValue::new(0));
            }
            Op::OP_PUSH_ONE => {
                vm.upush(JsValue::new(1));
            }
            Op::OP_PUSH_INT => {
                let i = pc.cast::<i32>().read_unaligned();
                pc = pc.add(4);
                vm.push(JsValue::new(i))
            }
            Op::OP_PUSH_LIT => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                vm.upush(*(*frame).bcode.unwrap().literals.get_unchecked(ix as usize));
            }
            Op::OP_LOGICAL_NOT => {
                let v1 = vm.upop();
                vm.upush(JsValue::new(v1.to_boolean()));
            }

            Op::OP_NOT => {
                let v1 = vm.upop();
                if v1.is_int32() {
                    vm.upush(JsValue::new(!v1.as_int32()));
                } else {
                    let n = v1.to_number(vm)?;
                    vm.push(JsValue::new(!(n as i32)));
                }
            }
            Op::OP_NEG => {
                let v1 = vm.upop();
                if v1.is_int32() {
                    vm.upush(JsValue::new(-v1.as_int32()));
                } else {
                    let n = v1.to_number(vm)?;
                    vm.upush(JsValue::new(-n));
                }
            }
            Op::OP_POS => {
                let v1 = vm.upop();
                let n = v1.to_number(vm)?;
                vm.upush(JsValue::new(n));
            }

            Op::OP_ADD => {
                let mut v2 = vm.pop().unwrap();
                let mut v1 = vm.pop().unwrap();

                if v1.is_cell() || v2.is_cell() {
                    v1 = v1.to_primitive(vm, JsHint::None)?;
                    v2 = v2.to_primitive(vm, JsHint::None)?;
                }
                // fast path
                if v1.is_int32() && v2.is_int32() {
                    if let Some(result) = v1.as_int32().checked_add(v2.as_int32()) {
                        vm.upush(JsValue::new(result));
                        continue;
                    }
                }
                // middle path
                if v1.is_number() && v2.is_number() {
                    let x = v1.number();
                    let y = v1.number();
                    vm.upush(JsValue::new(x + y));
                    /* everything other down there is slow path */
                } else if (v1.is_cell() && v1.as_cell().is::<JsString>())
                    || (v2.is_cell() && v2.as_cell().is::<JsString>())
                {
                    let s1 = v1.to_string(vm)?;
                    let s2 = v2.to_string(vm)?;
                    let res = JsString::new(vm, format!("{}{}", s1, s2));
                    vm.upush(JsValue::new(res));
                } else {
                    let v1 = v1.to_number(vm)?;
                    let v2 = v2.to_number(vm)?;
                    vm.upush(JsValue::new(v1 + v2));
                }
            }
            Op::OP_SUB => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                if v1.is_int32() && v2.is_int32() {
                    if let Some(result) = v1.as_int32().checked_sub(v2.as_int32()) {
                        vm.upush(JsValue::new(result));
                        continue;
                    }
                }

                let v1 = v1.to_number(vm)?;
                let v2 = v2.to_number(vm)?;
                vm.upush(JsValue::new(v1 - v2));
            }
            Op::OP_MUL => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                if v1.is_int32() && v2.is_int32() {
                    if let Some(result) = v1.as_int32().checked_mul(v2.as_int32()) {
                        vm.upush(JsValue::new(result));
                        continue;
                    }
                }

                let v1 = v1.to_number(vm)?;
                let v2 = v2.to_number(vm)?;
                vm.upush(JsValue::new(v1 * v2));
            }

            Op::OP_DIV => {
                // this opcode is special. We have int32 value for optimizing math
                // so we have to perform operation on doubles and only then try to
                // inline double into int32
                let v2 = vm.upop();
                let v1 = vm.upop();
                let v1 = v1.to_number(vm)?;
                let v2 = v2.to_number(vm)?;
                let result = v1 / v2;
                let val = if result as i32 as f64 == result {
                    JsValue::new(result as i32)
                } else {
                    JsValue::new(result)
                };
                vm.upush(val);
            }
            Op::OP_REM => {
                // this opcode is special. We have int32 value for optimizing math
                // so we have to perform operation on doubles and only then try to
                // inline double into int32
                let v2 = vm.upop();
                let v1 = vm.upop();
                let v1 = v1.to_number(vm)?;
                let v2 = v2.to_number(vm)?;
                let result = v1 % v2;
                let val = if result as i32 as f64 == result {
                    JsValue::new(result as i32)
                } else {
                    JsValue::new(result)
                };
            }
            Op::OP_LSHIFT => {
                let v2 = vm.upop();
                let v1 = vm.upop();

                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32();
                    let v2 = v2.as_int32();
                    let res = v1 << v2;
                    vm.upush(JsValue::new(res));
                } else {
                    let v1 = v1.to_number(vm)? as i32;
                    let v2 = v2.to_number(vm)? as i32;
                    vm.upush(JsValue::new(v1 << v2));
                }
            }
            Op::OP_RSHIFT => {
                let v2 = vm.upop();
                let v1 = vm.upop();

                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32();
                    let v2 = v2.as_int32() as u32;
                    let res = v1 >> v2;
                    vm.upush(JsValue::new(res));
                } else {
                    let v1 = v1.to_number(vm)? as i32;
                    let v2 = v2.to_number(vm)? as i32 as u32;
                    vm.upush(JsValue::new(v1 >> v2));
                }
            }
            Op::OP_URSHIFT => {
                let v2 = vm.upop();
                let v1 = vm.upop();

                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32() as u32;
                    let v2 = v2.as_int32() as u32;
                    let res = v1 >> v2;
                    vm.upush(JsValue::new(res));
                } else {
                    let v1 = v1.to_number(vm)? as i32 as u32;
                    let v2 = v2.to_number(vm)? as i32 as u32;
                    vm.upush(JsValue::new(v1 >> v2));
                }
            }
            Op::OP_OR => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32();
                    let v2 = v2.as_int32();
                    vm.upush(JsValue::new(v1 | v2));
                } else {
                    let v1 = v1.to_number(vm)?;
                    let v2 = v2.to_number(vm)?;
                    vm.upush(JsValue::new(v1 as i32 | v2 as i32));
                }
            }
            Op::OP_AND => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32();
                    let v2 = v2.as_int32();
                    vm.upush(JsValue::new(v1 & v2));
                } else {
                    let v1 = v1.to_number(vm)?;
                    let v2 = v2.to_number(vm)?;
                    vm.upush(JsValue::new(v1 as i32 & v2 as i32));
                }
            }
            Op::OP_XOR => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                if v1.is_int32() && v2.is_int32() {
                    let v1 = v1.as_int32();
                    let v2 = v2.as_int32();
                    vm.upush(JsValue::new(v1 ^ v2));
                } else {
                    let v1 = v1.to_number(vm)?;
                    let v2 = v2.to_number(vm)?;
                    vm.upush(JsValue::new(v1 as i32 ^ v2 as i32));
                }
            }

            Op::OP_LT => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v1.compare(v2, true, vm)? == CMP_TRUE;
                //println!("{} < {}", v1.to_string(vm)?, v2.to_string(vm)?);
                vm.upush(JsValue::new(res));
            }
            Op::OP_LE => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v2.compare(v1, false, vm)? == CMP_FALSE;
                vm.upush(JsValue::new(res));
            }
            Op::OP_GT => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v2.compare(v1, false, vm)? == CMP_TRUE;
                vm.upush(JsValue::new(res));
            }
            Op::OP_GE => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v1.compare(v2, true, vm)? == CMP_FALSE;
                vm.upush(JsValue::new(res));
            }
            Op::OP_EQ => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v1.abstract_equal(v2, vm)?;
                vm.upush(JsValue::new(res));
            }
            Op::OP_EQ_EQ => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = v1.strict_equal(v2);
                vm.upush(JsValue::new(res));
            }
            Op::OP_NE_NE => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = !v1.strict_equal(v2);
                vm.upush(JsValue::new(res));
            }
            Op::OP_NE => {
                let v2 = vm.upop();
                let v1 = vm.upop();
                let res = !v1.abstract_equal(v2, vm)?;
                vm.upush(JsValue::new(res));
            }

            Op::OP_GET_SCOPE => {
                vm.upush((*frame).scope);
            }
            Op::OP_POP_SCOPE => {
                let scope = (*frame).scope;
                let obj = scope.as_object();
                let proto = obj.prototype();
                if proto.is_none() {
                    let msg = JsString::new(vm, "can't pop scope");
                    return Err(JsValue::new(JsError::new(vm, msg, None)));
                }

                (*frame).scope = JsValue::new(proto.unwrap());
            }

            Op::OP_PUSH_SCOPE => {
                let scope = (*frame).scope;

                let structure = Structure::new_indexed(
                    vm,
                    if scope.is_object() {
                        Some(scope.as_object())
                    } else {
                        None
                    },
                    false,
                );
                (*frame).scope = JsValue::new(JsObject::new(
                    vm,
                    structure,
                    JsObject::get_class(),
                    ObjectTag::Ordinary,
                ));
            }
            Op::OP_JMP => {
                let offset = pc.cast::<i32>().read_unaligned();
                pc = pc.add(4);
                pc = pc.offset(offset as _);
            }
            Op::OP_JMP_FALSE => {
                let offset = pc.cast::<i32>().read_unaligned();
                pc = pc.add(4);
                let val = vm.upop();
                if !val.to_boolean() {
                    pc = pc.offset(offset as _);
                }
            }
            Op::OP_GET_VAR => {
                let ix = pc.cast::<u32>().read_unaligned(); // name id
                pc = pc.add(4);
                let nix = pc.cast::<u32>().read_unaligned(); // feedback vector id
                pc = pc.add(4);
                let name = bcode.names[ix as usize];
                let var = vm.bcode_get_var(name, (*frame).scope.as_object(), nix, bcode)?;
                assert!(!var.is_undefined());
                vm.push(var);
            }
            Op::OP_SET_VAR => {
                let val = vm.upop();
                let ix = pc.cast::<u32>().read_unaligned(); // name id
                pc = pc.add(4);
                let nix = pc.cast::<u32>().read_unaligned(); // feedback vector id
                pc = pc.add(4);
                let name = bcode.names[ix as usize];
                vm.bcode_set_var(
                    name,
                    (*frame).scope.as_object(),
                    nix,
                    val,
                    bcode.strict,
                    bcode,
                )?;
            }
            Op::OP_DECL_VAR => {
                let name = pc.cast::<u32>().read_unaligned();
                let name = bcode.names[name as usize];
                Env {
                    record: (*frame).scope.as_object(),
                }
                .declare_variable(vm, name, true)?;
            }

            Op::OP_RET => {
                let val = vm.upop();
                if LOG {
                    println!("leave frame {:p}", frame);
                }
                return Ok(val);
            }
            Op::OP_TRY_PUSH_CATCH => {
                let offset = pc.cast::<i32>().read_unaligned();
                pc = pc.add(4);
                let to = pc.offset(offset as _);
                (*frame).try_stack.push(((*frame).scope.as_object(), to));
            }
            Op::OP_TRY_POP => {
                (*frame).try_stack.pop();
            }
            Op::OP_ENTER_CATCH => {}
            Op::OP_THROW => {
                // exception is handled in upper function.
                let v1 = vm.upop();
                return Err(v1);
            }
            Op::OP_GET_FUNCTION => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let func =
                    JsVMFunction::new(vm, bcode.codes[ix as usize], (*frame).scope.as_object());
                vm.upush(JsValue::new(func));
            }
            Op::OP_CALL | Op::OP_NEW => {
                let mut argc = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let is_ctor = op == Op::OP_NEW;
                let v1 = vm.upop(); // func
                let mut v3 = vm.upop(); // this
                if v3.is_empty() {
                    v3 = JsValue::new(vm.global_object());
                }

                let mut args = ctx.new_local(Arguments::new(vm, v3, argc as _));
                let mut i = 0;
                while argc > 0 {
                    args[i] = vm.upop();
                    assert!(!args[i].is_empty());
                    i += 1;
                    argc -= 1;
                }

                if !v1.is_callable() {
                    let msg = JsString::new(vm, "tried to call non function object");
                    return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
                }
                args.ctor_call = is_ctor;
                let mut obj = v1.as_object();
                //let f = obj.as_function_mut();

                let result = if is_ctor {
                    let s = match obj.func_construct_map(vm) {
                        Ok(val) => Some(val),
                        _ => None,
                    };
                    obj.as_function_mut().construct(vm, &ctx, &mut args, s)?
                } else {
                    obj.as_function_mut().call(vm, &ctx, &mut args)?
                };
                if op == Op::OP_NEW {
                    assert!(result.is_object());
                }

                vm.upush(result);
            }
            Op::OP_PUSH_EMPTY => {
                vm.upush(JsValue::empty());
            }
            Op::OP_GET => {
                let obj = vm.upop();
                let name = vm.upop();
                let sym = name.to_symbol(vm)?;
                let val = vm.get_(obj, sym)?;
                vm.upush(val);
            }
            Op::OP_SET => {
                let obj = vm.upop();
                let name = vm.upop();
                let val = vm.upop();
                let sym = name.to_symbol(vm)?;
                vm.put_(obj, sym, val, bcode.strict)?;
            }
            Op::OP_SET_PROP => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let fix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let name = bcode.names[ix as usize];
                let obj = vm.upop();
                let val = vm.upop();
                vm.set_prop(obj, name, val, fix, bcode.strict, bcode)?;
            }
            Op::OP_GET_PROP => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let fix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let name = bcode.names[ix as usize];
                let obj = vm.upop();
                let val = vm.get_prop(obj, name, fix, bcode.strict, bcode)?;
                vm.upush(val);
            }
            Op::OP_DECL_LET => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let _fix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);

                // TODO: PolyIC for decl_let
                let mut env = Env {
                    record: (*frame).scope.as_object(),
                };
                let name = bcode.names[ix as usize];
                env.declare_variable(vm, name, true)?;
            }
            Op::OP_DECL_IMMUTABLE => {
                let ix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);
                let _fix = pc.cast::<u32>().read_unaligned();
                pc = pc.add(4);

                // TODO: PolyIC for decl_const
                let mut env = Env {
                    record: (*frame).scope.as_object(),
                };
                let name = bcode.names[ix as usize];
                env.declare_variable(vm, name, false)?;
            }
            _ => todo!("unimplemented or unknown opcode {:?}", op),
        }
    }
}

unsafe fn eval_internal(
    vm: &mut VirtualMachine,
    bcode: Gc<ByteCode>,
    pc: *mut u8,
    this: JsValue,
    mut scope: Gc<JsObject>,
) -> Result<JsValue, JsValue> {
    let mut frame = vm.init_call_frame_bcode(bcode, JsValue::new(scope), this, pc, false);
    for val in bcode.var_names.iter() {
        scope.put(vm, *val, JsValue::undefined(), false)?;
    }
    (*frame).code = bcode.code_start;
    loop {
        match eval_bcode(vm, frame) {
            Ok(val) => {
                let frame = Box::from_raw(frame);
                vm.frame = frame.prev;
                return Ok(val);
            }
            Err(e) => match (*frame).try_stack.pop() {
                Some(addr) => {
                    (*frame).code = addr.1 as *mut u8;
                    (*frame).scope = JsValue::new(addr.0);

                    vm.upush(e);
                    continue;
                }
                None => return Err(e),
            },
        }
    }
}

impl VirtualMachine {
    #[allow(clippy::explicit_counter_loop)]
    pub(crate) fn perform_vm_call(
        &mut self,
        func: &JsVMFunction,
        env: JsValue,
        args_: &Arguments,
    ) -> Result<JsValue, JsValue> {
        unsafe {
            let f = func;
            let scope = env.as_object();
            let mut nscope = JsObject::new(
                self,
                scope.structure(),
                JsObject::get_class(),
                ObjectTag::Ordinary,
            );
            let mut i = 0;

            for p in f.code.params.iter() {
                let _ = nscope
                    .put(self, *p, args_[i], false)
                    .unwrap_or_else(|_| panic!());

                i += 1;
            }

            let args = JsArguments::new(self, nscope, &f.code.params);
            let _ = nscope.put(self, Symbol::arguments(), JsValue::new(args), false);
            let mut slot = Slot::new();
            let _slot = nscope
                .get_slot(self, Symbol::arguments(), &mut slot)
                .unwrap_or_else(|_| panic!());

            eval_internal(self, f.code, f.code.code_start, args_.this, nscope)
        }
    }
    fn bcode_get_var(
        &mut self,
        name: Symbol,
        scope: Gc<JsObject>,
        feedback: u32,
        mut bcode: Gc<ByteCode>,
    ) -> Result<JsValue, JsValue> {
        match &bcode.feedback[feedback as usize] {
            TypeFeedBack::Generic => return Env { record: scope }.get_variable(self, name),
            TypeFeedBack::Structure(structure, offset, count) => {
                let count = *count;
                let structure = *structure;
                let offset = *offset;

                if let Some(hit) = self.try_cache(structure, scope) {
                    return Ok(*hit.direct(offset as usize));
                } else {
                    if count == 64 {
                        bcode.feedback[feedback as usize] = TypeFeedBack::Generic;
                        Env { record: scope }.get_variable(self, name)
                    } else {
                        let mut slot = Slot::new();
                        let val = Env { record: scope }.get_variable_slot(self, name, &mut slot)?;

                        if slot.is_load_cacheable() {
                            unsafe {
                                bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                                    slot.base()
                                        .unwrap()
                                        .downcast_unchecked::<JsObject>()
                                        .structure(),
                                    slot.offset(),
                                    count + 1,
                                );
                            }
                        }
                        Ok(val)
                    }
                }
            }
            TypeFeedBack::None => {
                let mut slot = Slot::new();
                let val = Env { record: scope }.get_variable_slot(self, name, &mut slot)?;

                if slot.is_load_cacheable() {
                    unsafe {
                        bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                            slot.base()
                                .unwrap()
                                .downcast_unchecked::<JsObject>()
                                .structure(),
                            slot.offset(),
                            0,
                        );
                    }
                }
                Ok(val)
            }
            _ => unreachable!(),
        }
    }
    fn bcode_set_var(
        &mut self,
        name: Symbol,
        scope: Gc<JsObject>,
        feedback: u32,
        val: JsValue,
        strict: bool,
        mut bcode: Gc<ByteCode>,
    ) -> Result<(), JsValue> {
        match &bcode.feedback[feedback as usize] {
            TypeFeedBack::Generic => {
                Env { record: scope }.set_variable(self, name, val, strict)?;
                Ok(())
            }
            TypeFeedBack::Structure(structure, offset, count) => {
                let count = *count;
                let structure = *structure;
                let offset = *offset;
                if let Some(mut hit) = self.try_cache(structure, scope) {
                    *hit.direct_mut(offset as _) = val;
                } else {
                    if count == 64 {
                        bcode.feedback[feedback as usize] = TypeFeedBack::Generic;
                        Env { record: scope }.set_variable(self, name, val, strict)?;
                    } else {
                        let (base, slot) =
                            Env { record: scope }.set_variable(self, name, val, strict)?;
                        if slot.is_store_cacheable() {
                            unsafe {
                                bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                                    base.structure(),
                                    slot.offset(),
                                    count + 1,
                                );
                            }
                        }
                    }
                }
                Ok(())
            }
            TypeFeedBack::None => {
                let (base, slot) = Env { record: scope }.set_variable(self, name, val, strict)?;
                if slot.is_store_cacheable() {
                    unsafe {
                        bcode.feedback[feedback as usize] =
                            TypeFeedBack::Structure(base.structure(), slot.offset(), 0);
                    }
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }
    fn create_call_frame(&mut self, scope: JsValue) -> *mut FrameBase {
        let mut frame = Box::new(FrameBase {
            prev: self.frame,
            scope,
            try_stack: vec![],
            bcode: None,
            code: null_mut(),
            is_bcode: 0,
            is_ctor: 0,
            is_thrown: 0,
            stack_size: 0,
            this_obj: JsValue::undefined(),
            thrown_val: JsValue::undefined(),
            callee: JsValue::undefined(),
        });

        let p = Box::into_raw(frame);
        self.frame = p;
        p
    }
    fn init_call_frame_bcode(
        &mut self,
        bcode: Gc<ByteCode>,
        scope: JsValue,
        this: JsValue,
        pc: *mut u8,
        is_ctor: bool,
    ) -> *mut FrameBase {
        let frame = self.create_call_frame(scope);
        unsafe {
            (*frame).this_obj = this;
            (*frame).bcode = Some(bcode);
            (*frame).code = pc;
            (*frame).is_ctor = is_ctor as _;
            (*frame).is_bcode = 1;
        }
        frame
    }

    fn put_(
        &mut self,
        obj: JsValue,
        name: Symbol,
        val: JsValue,
        strict: bool,
    ) -> Result<(), JsValue> {
        let mut obj = if obj.is_object() {
            obj.as_object()
        } else {
            obj.get_primitive_proto(self)
        };
        obj.put(self, name, val, strict)
    }

    fn get_(&mut self, obj: JsValue, name: Symbol) -> Result<JsValue, JsValue> {
        let mut obj = if obj.is_object() {
            obj.as_object()
        } else {
            obj.get_primitive_proto(self)
        };
        obj.get(self, name)
    }

    fn get_prop(
        &mut self,
        obj: JsValue,
        name: Symbol,
        feedback: u32,
        strict: bool,
        mut bcode: Gc<ByteCode>,
    ) -> Result<JsValue, JsValue> {
        match &bcode.feedback[feedback as usize] {
            TypeFeedBack::Generic => {
                let mut slot = Slot::new();
                return obj.get_slot(self, name, &mut slot);
            }
            TypeFeedBack::None => {
                let mut slot = Slot::new();
                let val = obj.get_slot(self, name, &mut slot)?;

                if slot.is_load_cacheable() {
                    bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                        slot.base()
                            .unwrap()
                            .downcast::<JsObject>()
                            .unwrap()
                            .structure(),
                        slot.offset(),
                        0,
                    );
                }
                return Ok(val);
            }
            TypeFeedBack::Structure(structure, offset, count) => {
                let structure = *structure;
                let offset = *offset;
                let count = *count;
                let obj = if obj.is_object() {
                    obj.as_object()
                } else {
                    obj.get_primitive_proto(self)
                };
                if let Some(hit) = self.try_cache(structure, obj) {
                    return Ok(*hit.direct(offset as _));
                } else {
                    if count == 64 {
                        bcode.feedback[feedback as usize] = TypeFeedBack::Generic;
                        return obj.get(self, name);
                    } else {
                        let mut slot = Slot::new();
                        let val = obj.get_slot(self, name, &mut slot)?;
                        if slot.is_load_cacheable() {
                            bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                                slot.base()
                                    .unwrap()
                                    .downcast::<JsObject>()
                                    .unwrap()
                                    .structure(),
                                slot.offset(),
                                count + 1,
                            );
                        }
                        return Ok(val);
                    }
                }
            }
            _ => unreachable!(),
        }
    }
    fn set_prop(
        &mut self,
        obj: JsValue,
        name: Symbol,
        val: JsValue,
        feedback: u32,
        strict: bool,
        mut bcode: Gc<ByteCode>,
    ) -> Result<(), JsValue> {
        let mut obj = if obj.is_object() {
            obj.as_object()
        } else {
            obj.get_primitive_proto(self)
        };
        match &bcode.feedback[feedback as usize] {
            TypeFeedBack::Generic => obj.put(self, name, val, strict),
            TypeFeedBack::None => {
                let mut slot = Slot::new();
                let val = obj.put_slot(self, name, val, &mut slot, strict)?;
                if slot.is_store_cacheable() {
                    bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                        slot.base()
                            .unwrap()
                            .downcast::<JsObject>()
                            .unwrap()
                            .structure(),
                        slot.offset(),
                        0,
                    );
                }
                return Ok(());
            }
            TypeFeedBack::Structure(structure, offset, count) => {
                let structure = *structure;
                let offset = *offset;
                let count = *count;

                if let Some(hit) = self.try_cache(structure, obj) {
                    *obj.direct_mut(offset as _) = val;
                    return Ok(());
                } else {
                    if count == 64 {
                        bcode.feedback[feedback as usize] = TypeFeedBack::Generic;
                        return obj.put(self, name, val, strict);
                    } else {
                        let mut slot = Slot::new();
                        let val = obj.get_slot(self, name, &mut slot)?;
                        if slot.is_store_cacheable() {
                            bcode.feedback[feedback as usize] = TypeFeedBack::Structure(
                                slot.base()
                                    .unwrap()
                                    .downcast::<JsObject>()
                                    .unwrap()
                                    .structure(),
                                slot.offset(),
                                count + 1,
                            );
                        }

                        obj.put(self, name, val, strict)
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}
