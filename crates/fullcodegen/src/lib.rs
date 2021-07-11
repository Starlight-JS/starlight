use std::{any::TypeId, intrinsics::transmute};

use cranelift::{
    frontend::{FunctionBuilder, Variable},
    prelude::{types, InstBuilder, IntCC, MemFlags, Value},
};
use starlight::offsetof;
use starlight::prelude::*;
use starlight::{bytecode::profile::ArithProfile, gc::cell::*};

/// Full codegen JIT aka single-pass JIT.
pub struct FullCodegenBuilder<'a> {
    builder: FunctionBuilder<'a>,
    sp_var: Variable,
}

impl<'a> FullCodegenBuilder<'a> {
    pub fn pop(&mut self) -> Value {
        let sp = self.builder.use_var(self.sp_var);
        let new_sp = self.builder.ins().iadd_imm(sp, -8);
        let val = self
            .builder
            .ins()
            .load(types::I64, MemFlags::new(), new_sp, 0);
        self.builder.def_var(self.sp_var, new_sp);
        val
    }

    pub fn push(&mut self, val: Value) {
        let sp = self.builder.use_var(self.sp_var);
        self.builder.ins().store(MemFlags::new(), val, sp, 0);
        let new_sp = self.builder.ins().iadd_imm(sp, 8);
        self.builder.def_var(self.sp_var, new_sp);
    }

    pub fn empty_value(&mut self) -> Value {
        self.builder
            .ins()
            .iconst(types::I64, JsValue::VALUE_EMPTY as i64)
    }
    pub fn encode_object_value(&mut self, ptr: Value) -> Value {
        ptr
    }

    pub fn encode_undefined_value(&mut self) -> Value {
        self.builder
            .ins()
            .iconst(types::I64, JsValue::VALUE_UNDEFINED as i64)
    }
    pub fn encode_null_value(&mut self) -> Value {
        self.builder
            .ins()
            .iconst(types::I64, JsValue::VALUE_NULL as i64)
    }
    pub fn encode_bool_value(&mut self, x: Value) -> Value {
        let val = self.builder.ins().iconst(types::I8, 1);
        let true_ = self.builder.create_block();
        let false_ = self.builder.create_block();
        let merge = self.builder.create_block();
        self.builder.append_block_param(merge, types::I64);
        self.builder
            .ins()
            .br_icmp(IntCC::NotEqual, val, x, false_, &[]);
        self.builder.ins().fallthrough(true_, &[]);
        self.builder.switch_to_block(true_);
        let c = self
            .builder
            .ins()
            .iconst(types::I64, JsValue::VALUE_TRUE as i64);
        self.builder.ins().jump(merge, &[c]);
        self.builder.switch_to_block(false_);
        let c = self
            .builder
            .ins()
            .iconst(types::I64, JsValue::VALUE_FALSE as i64);
        self.builder.ins().jump(merge, &[c]);
        self.builder.switch_to_block(merge);
        self.builder.block_params(merge)[0]
    }

    pub fn is_undefined(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .icmp_imm(IntCC::Equal, val, JsValue::VALUE_UNDEFINED as i64);
        self.builder.ins().bint(types::I64, x)
    }

    pub fn is_null(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .icmp_imm(IntCC::Equal, val, JsValue::VALUE_NULL as i64);
        self.builder.ins().bint(types::I64, x)
    }

    pub fn is_true(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .icmp_imm(IntCC::Equal, val, JsValue::VALUE_TRUE as i64);
        self.builder.ins().bint(types::I64, x)
    }

    pub fn is_false(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .icmp_imm(IntCC::Equal, val, JsValue::VALUE_FALSE as i64);
        self.builder.ins().bint(types::I64, x)
    }

    pub fn is_boolean(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, !1i64);
        self.is_false(x)
    }

    pub fn is_pointer(&mut self, val: Value) -> Value {
        let x = self
            .builder
            .ins()
            .band_imm(val, JsValue::NOT_CELL_MASK as i64);
        let cmp = self.builder.ins().icmp_imm(IntCC::Equal, x, 0);
        self.builder.ins().bint(types::I64, cmp)
    }

    pub fn is_int32(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, JsValue::NUMBER_TAG as i64);
        let cmp = self
            .builder
            .ins()
            .icmp_imm(IntCC::Equal, x, JsValue::NUMBER_TAG);
        self.builder.ins().bint(types::I64, cmp)
    }

    pub fn is_number(&mut self, val: Value) -> Value {
        let x = self.builder.ins().band_imm(val, JsValue::NUMBER_TAG as i64);
        let cmp = self
            .builder
            .ins()
            .icmp_imm(IntCC::NotEqual, x, JsValue::NUMBER_TAG);
        self.builder.ins().bint(types::I64, cmp)
    }
    pub fn get_object(&mut self, val: Value) -> Value {
        val
    }

    pub fn is_object(&mut self, val: Value) -> Value {
        self.is_pointer(val)
    }

    pub fn is_jsobject(&mut self, val: Value) -> Value {
        let is_pointer = self.is_pointer(val);

        let merge = self.builder.create_block();
        self.builder.append_block_param(merge, types::I64);
        let c = self.builder.ins().iconst(types::I64, 0);
        self.builder.ins().brz(is_pointer, merge, &[c]);
        let type_id = self.builder.ins().load(
            types::I64,
            MemFlags::new(),
            val,
            GcPointerBase::typeid_offsetof() as i32,
        );
        let cmp_result = self.builder.ins().icmp_imm(IntCC::Equal, type_id, unsafe {
            transmute::<_, i64>(TypeId::of::<JsObject>())
        });
        let promoted = self.builder.ins().bint(types::I64, cmp_result);
        self.builder.ins().jump(merge, &[promoted]);
        self.builder.switch_to_block(merge);

        self.builder.block_params(merge)[0]
    }
}
