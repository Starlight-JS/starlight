#![allow(dead_code, unused_imports, unused_variables)]
use crate::{
    bytecode::opcodes::Opcode,
    prelude::*,
    vm::{
        code_block::CodeBlock,
        environment::*,
        interpreter::frame::*,
        value::{ExtendedTag, BOOL_TAG, FIRST_TAG, OBJECT_TAG},
    },
};
use gccjit_rs::{
    block::{BinaryOp, Block, Case, ComparisonOp},
    function::Function,
    lvalue::LValue,
    parameter::Parameter,
    rvalue::{RValue, ToRValue},
    ty::*,
};
use gccjit_rs::{ctx::Context, field::Field};
use std::{collections::HashMap, intrinsics::transmute};

pub mod stubs;

pub struct JITResult {
    pub is_err: i32,
    pub value: JsValue,
}

impl Typeable for JITResult {
    fn get_type(ctx: &Context) -> Type {
        let is_err = ctx.new_type::<i32>();
        let value;
        #[cfg(feature = "val-as-u64")]
        {
            value = ctx.new_type::<u64>()
        };
        #[cfg(feature = "val-as-f64")]
        {
            value = ctx.new_type::<f64>()
        }
        let is_err = ctx.new_field(None, is_err, "isErr");
        let value = ctx.new_field(None, value, "value");
        ctx.new_struct_type(None, "JITResult", &[is_err, value])
            .as_type()
    }
}

pub struct JITCompiler {
    op_blocks: HashMap<*mut u8, Block>,
    block_to_case: HashMap<*mut u8, Case>,
    pub ctx: Context,
    pub fun: Function,
    #[cfg(feature = "val-as-f64")]
    cast_union: Type,
}

impl JITCompiler {
    pub fn new(ctx: Context, name: &str, params: &[Parameter], ret: Type) -> Self {
        let fun = ctx.new_function(
            None,
            gccjit_rs::function::FunctionType::Exported,
            ret,
            params,
            name,
            false,
        );
        Self {
            block_to_case: HashMap::new(),
            fun,
            #[cfg(feature = "val-as-f64")]
            cast_union: ctx.new_type::<u64>(),
            ctx,
            op_blocks: Default::default(),
        }
    }
    pub fn ty_u64(&self) -> Type {
        self.ctx.new_type::<u64>()
    }
    pub fn u64(&self, x: u64) -> RValue {
        self.ctx.new_rvalue_from_long(self.ty_u64(), x as _)
    }
    pub fn val_raw(&self, val: impl ToRValue) -> RValue {
        #[cfg(feature = "val-as-u64")]
        {
            val.to_rvalue()
        }
        #[cfg(feature = "val-as-f64")]
        {
            todo!()
        }
    }
    pub fn get_tag(&self, val: impl ToRValue) -> RValue {
        let val = self.val_raw(val);
        val >> self.u64(JsValue::NUM_DATA_BITS as _)
    }

    pub fn get_etag(&self, val: impl ToRValue) -> RValue {
        val.to_rvalue() >> self.u64(JsValue::NUM_DATA_BITS as u64 - 1)
    }
    pub fn combine_tags(&self, a: RValue, b: RValue) -> RValue {
        ((a & self.u64(JsValue::TAG_MASK as u64)) << self.u64(JsValue::TAG_WIDTH as u64))
            | (b & self.u64(JsValue::TAG_MASK as _))
    }

    pub fn val_new(&self, val: RValue, tag: RValue) -> RValue {
        val | (tag << self.u64(JsValue::NUM_DATA_BITS as _))
    }

    pub fn new_extended(&self, val: RValue, tag: RValue) -> RValue {
        val | (tag << self.u64(JsValue::NUM_DATA_BITS as u64 - 1))
    }

    pub fn encode_object_value(&self, val: RValue) -> RValue {
        self.val_new(
            self.ctx.new_cast(None, val, self.ctx.new_type::<u64>()),
            self.u64(OBJECT_TAG as _),
        )
    }

    pub fn encode_bool_value(&self, val: impl ToRValue) -> RValue {
        self.val_new(
            self.ctx
                .new_cast(None, val.to_rvalue(), self.ctx.new_type::<u64>()),
            self.u64(BOOL_TAG as _),
        )
    }

    pub fn encode_undefined_value(&self) -> RValue {
        self.new_extended(self.u64(0), self.u64(ExtendedTag::Undefined as _))
    }

    pub fn encode_null_value(&self) -> RValue {
        self.new_extended(self.u64(0), self.u64(ExtendedTag::Null as _))
    }

    pub fn bitcast(&self, block: &Block, val: impl ToRValue, to: Type) -> RValue {
        let val = val.to_rvalue();
        let local = self.fun.new_local(None, val.get_type(), "cast");
        block.add_assignment(None, local, val);
        let addr = local.get_address(None);
        let cast = self.ctx.new_cast(None, addr, to.make_pointer());
        cast.dereference(None).to_rvalue()
    }

    pub fn encode_f64_value(&self, block: &Block, val: impl ToRValue) -> RValue {
        let val = self.bitcast(block, val.to_rvalue(), self.ctx.new_type::<u64>());
        val
    }

    pub fn encode_nan_value(&self, _block: &Block) -> RValue {
        self.ctx
            .new_rvalue_from_long(self.ctx.new_type::<u64>(), 0x7ff8000000000000u64 as i64)
    }

    pub fn is_null(&self, val: impl ToRValue) -> RValue {
        self.ctx.new_comparison(
            None,
            ComparisonOp::Equals,
            self.get_etag(val),
            self.u64(ExtendedTag::Null as _),
        )
    }

    pub fn is_undefined(&self, val: impl ToRValue) -> RValue {
        self.ctx.new_comparison(
            None,
            ComparisonOp::Equals,
            self.get_etag(val),
            self.u64(ExtendedTag::Undefined as _),
        )
    }

    pub fn is_bool(&self, val: impl ToRValue) -> RValue {
        self.ctx.new_comparison(
            None,
            ComparisonOp::Equals,
            self.get_tag(val),
            self.u64(BOOL_TAG as _),
        )
    }

    pub fn is_object(&self, val: impl ToRValue) -> RValue {
        self.ctx.new_comparison(
            None,
            ComparisonOp::Equals,
            self.get_tag(val),
            self.u64(OBJECT_TAG as _),
        )
    }

    pub fn is_double(&self, val: impl ToRValue) -> RValue {
        let val = self.val_raw(val.to_rvalue());
        self.ctx.new_comparison(
            None,
            ComparisonOp::LessThan,
            val,
            self.u64((FIRST_TAG as u64) << JsValue::NUM_DATA_BITS as u64),
        )
    }

    pub fn get_double(&self, block: &Block, val: impl ToRValue) -> RValue {
        let val = self.val_raw(val.to_rvalue());
        self.bitcast(block, val, self.ctx.new_type::<f64>())
    }

    pub fn get_bool(&self, val: impl ToRValue) -> RValue {
        self.ctx.new_comparison(
            None,
            ComparisonOp::NotEquals,
            val.to_rvalue() & self.u64(0x1),
            self.u64(0),
        )
    }

    pub fn get_object(&self, val: impl ToRValue) -> RValue {
        let val = val.to_rvalue() & self.u64(JsValue::DATA_MASK as _);
        self.ctx
            .new_cast(None, val, self.ctx.new_type::<()>().make_pointer())
    }
    /// Check if `val`'s type id is equal to other type id.
    ///
    /// NOTE:  `val` must be pointer to GC allocated object.
    pub fn check_type_equals(&self, val: impl ToRValue, type_id: impl ToRValue) -> RValue {
        let ret = self.ctx.new_type::<u64>();
        let param = self.ctx.new_type::<*const ()>();
        let ty = self
            .ctx
            .new_function_pointer_type(None, ret, &[param], false);
        // #1: Invoke stub that obtains object type id.
        let func = self
            .ctx
            .new_rvalue_from_ptr(ty, stubs::type_id_of_object_stub as *mut ());
        let val = self
            .ctx
            .new_cast(None, val.to_rvalue(), self.ctx.new_type::<*const ()>());
        let result = self.ctx.new_call_through_ptr(None, func, &[val]);
        // #2: compare type id of object and desired type id
        self.ctx
            .new_comparison(None, ComparisonOp::Equals, result, type_id.to_rvalue())
    }
    pub fn push(&mut self, block: &Block, frame: LValue, value: RValue) {
        let sp = frame.access_field(
            None,
            self.ctx
                .new_field(None, self.ctx.new_type::<u64>().make_pointer(), "sp"),
        );
        block.add_assignment(None, sp.get_address(None).dereference(None), value);
        block.add_assignment_op(None, sp, BinaryOp::Plus, self.u64(1));
    }
    pub fn pop(&mut self, block: &Block, frame: LValue) -> LValue {
        let sp = frame.access_field(
            None,
            self.ctx
                .new_field(None, self.ctx.new_type::<u64>().make_pointer(), "sp"),
        );
        block.add_assignment_op(None, sp, BinaryOp::Minus, self.u64(1));
        sp.get_address(None).dereference(None)
    }

    pub fn pop_or_undefined(&mut self, block: &mut Block, frame: LValue) -> RValue {
        let merge = self
            .fun
            .new_local(None, self.ctx.new_type::<u64>(), "merge");
        let sp = frame.access_field(
            None,
            self.ctx
                .new_field(None, self.ctx.new_type::<u64>().make_pointer(), "sp"),
        );
        let limit = frame.access_field(
            None,
            self.ctx
                .new_field(None, self.ctx.new_type::<u64>().make_pointer(), "limit"),
        );
        let cond = self.ctx.new_comparison(
            None,
            ComparisonOp::LessThanEquals,
            sp.to_rvalue(),
            limit.to_rvalue(),
        );
        let if_true = self.fun.new_block("pop_undef");
        let if_false = self.fun.new_block("real_pop");
        let merge_block = self.fun.new_block("merge");
        block.end_with_conditional(None, cond, if_true, if_false);
        *block = if_false;
        block.add_assignment_op(None, sp, BinaryOp::Minus, self.u64(1));
        block.add_assignment(
            None,
            merge,
            sp.get_address(None).dereference(None).to_rvalue(),
        );
        block.end_with_jump(None, merge_block);
        *block = if_true;
        block.add_assignment(None, merge, self.encode_undefined_value());
        block.end_with_jump(None, merge_block);
        *block = merge_block;
        merge.to_rvalue()
    }

    pub fn get_or_add_block(&mut self, ip: *mut u8) -> Block {
        if let Some(bb) = self.op_blocks.get(&ip) {
            return *bb;
        }
        let block = self.fun.new_block("bb");
        self.op_blocks.insert(ip, block);
        block
    }
    pub fn generate(
        &mut self,
        mut code_block: GcPointer<CodeBlock>,
        frame_struct: gccjit_rs::structs::Struct,
    ) {
        let mut pc: *mut u8 = &mut code_block.code[0];
        let pframe = self.fun.get_param(0);
        let pruntime = self.fun.get_param(1);
        let ptarget = self.fun.get_param(2);
        let frame = self
            .fun
            .new_local(None, self.ctx.new_type::<()>().make_pointer(), "frame");
        let runtime = self
            .fun
            .new_local(None, self.ctx.new_type::<()>().make_pointer(), "runtime");
        let target = self
            .fun
            .new_local(None, self.ctx.new_type::<u32>(), "target");
        let entry = self.fun.new_block("entry");
        entry.add_assignment(None, frame, pframe.to_rvalue());
        entry.add_assignment(None, runtime, pruntime.to_rvalue());
        entry.add_assignment(None, target, ptarget.to_rvalue());
        let end = code_block.code.last_mut().unwrap() as *mut u8;
        unsafe {
            while pc < end {
                let op_loc = pc;
                let op = transmute::<_, Opcode>(pc.read());
                let block = self.get_or_add_block(pc);
                pc = pc.add(1);
                match op {
                    Opcode::OP_NOP => { /* no op */ }
                    Opcode::OP_POP => {
                        self.pop(&block, frame);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_UNDEF => {
                        let undef = self.encode_undefined_value();
                        self.push(&block, frame, undef);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_NULL => {
                        let null = self.encode_null_value();
                        self.push(&block, frame, null);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_NAN => {
                        let nan = self.encode_nan_value(&block);
                        self.push(&block, frame, nan);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_TRUE => {
                        let val = self.encode_bool_value(
                            self.ctx.new_rvalue_from_int(self.ctx.new_type::<bool>(), 1),
                        );
                        self.push(&block, frame, val);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_FALSE => {
                        let val = self.encode_bool_value(
                            self.ctx.new_rvalue_from_int(self.ctx.new_type::<bool>(), 1),
                        );
                        self.push(&block, frame, val);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_PUSH_ENV => {
                        let fty = self.ctx.new_function_pointer_type(
                            None,
                            self.ctx.new_type::<()>(),
                            &[
                                self.ctx.new_type::<()>().make_pointer(),
                                frame_struct.as_type(),
                            ],
                            false,
                        );
                        let fptr = self
                            .ctx
                            .new_rvalue_from_ptr(fty, stubs::push_env as *mut ());
                        let call = self.ctx.new_call_through_ptr(
                            None,
                            fptr,
                            &[runtime.to_rvalue(), frame.to_rvalue()],
                        );
                        block.add_eval(None, call);
                        //  let field = self.ctx.new_field(None, self.ctx.new_type::<u64>(), "env");
                        //  let
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_POP_ENV => {
                        let field = self.ctx.new_field(None, self.ctx.new_type::<u64>(), "env");

                        let env = frame.access_field(None, field);
                        let unboxed = self.get_object(env.to_rvalue());
                        let parent = self.ctx.new_binary_op(
                            None,
                            BinaryOp::Plus,
                            self.ctx.new_type::<()>().make_pointer(),
                            unboxed,
                            self.ctx.new_rvalue_from_long(
                                self.ctx.new_type::<usize>(),
                                gc_offsetof!(Environment.parent) as _,
                            ),
                        );
                        let boxed = self.encode_object_value(parent);
                        block.add_assignment(None, env, boxed);
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_GET_ENV => {
                        let field = self.ctx.new_field(None, self.ctx.new_type::<u64>(), "env");
                        let depth = pc.cast::<u32>().read_unaligned();
                        pc = pc.add(4);
                        let env = frame.access_field(None, field);
                        if depth == 0 {
                            self.push(&block, frame, env.to_rvalue());
                        } else {
                            let level =
                                self.fun
                                    .new_local(None, self.ctx.new_type::<u32>(), "level");
                            block.add_assignment(
                                None,
                                level,
                                self.ctx
                                    .new_rvalue_from_int(self.ctx.new_type::<u32>(), depth as _),
                            );

                            let env_ = self.fun.new_local(
                                None,
                                self.ctx.new_type::<()>().make_pointer(),
                                "envx",
                            );
                            block.add_assignment(None, env_, self.get_object(env.to_rvalue()));
                            let loop_cond = self.fun.new_block("loop");
                            let loop_body = self.fun.new_block("body");
                            let after_loop = self.fun.new_block("after_loop");

                            block.end_with_jump(None, loop_cond);
                            let res = self.ctx.new_comparison(
                                None,
                                ComparisonOp::NotEquals,
                                level.to_rvalue(),
                                self.ctx.new_rvalue_from_int(self.ctx.new_type::<u32>(), 0),
                            );
                            loop_cond.end_with_conditional(None, res, loop_body, after_loop);

                            loop_body.add_assignment_op(
                                None,
                                level,
                                BinaryOp::Minus,
                                self.ctx.new_rvalue_from_int(self.ctx.new_type::<u32>(), 0),
                            );
                            let loaded = self.ctx.new_binary_op(
                                None,
                                BinaryOp::Plus,
                                self.ctx.new_type::<()>().make_pointer(),
                                env_.to_rvalue(),
                                self.ctx.new_rvalue_from_long(
                                    self.ctx.new_type::<usize>(),
                                    gc_offsetof!(Environment.parent) as _,
                                ),
                            );
                            let loaded = self.ctx.new_cast(
                                None,
                                loaded,
                                self.ctx.new_type::<()>().make_pointer().make_pointer(),
                            );
                            loop_body.add_assignment(None, env_, loaded.dereference(None));
                            loop_body.end_with_jump(None, loop_cond);

                            self.push(
                                &&after_loop,
                                frame,
                                self.encode_object_value(env_.to_rvalue()),
                            );
                        }
                        let next = self.get_or_add_block(pc);
                        block.end_with_jump(None, next);
                    }
                    Opcode::OP_RET => {}
                    _ => todo!("NYI: {:?}", op),
                }
            }
        }
    }
}
