use std::collections::HashMap;

use crate::{
    prelude::*,
    vm::value::{ExtendedTag, BOOL_TAG, FIRST_TAG, OBJECT_TAG},
};
use gccjit_rs::{
    block::{Block, ComparisonOp},
    function::Function,
    parameter::Parameter,
    rvalue::{RValue, ToRValue},
    ty::*,
};
use gccjit_rs::{ctx::Context, field::Field};

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
            fun,
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
}
