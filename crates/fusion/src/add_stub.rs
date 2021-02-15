use crate::compiler::InterpreterCompiler;

use cranelift::prelude::*;

pub struct AddStub;

impl AddStub {
    pub fn generate(
        ic: &mut InterpreterCompiler<'_>,
        frame: Value,
        left: Value,
        right: Value,
    ) -> Value {
        let left_not_int = ic.create_block();
        let right_not_int = ic.create_block();
        let finish = ic.create_block();
        let slowpath = ic.create_block();
        let right_is_double = ic.create_block();
        let right_was_integer = ic.create_block();
        let fop = ic.create_block();
        ic.append_block_param(right_was_integer, types::F64);
        ic.append_block_param(right_was_integer, types::F64);
        ic.append_block_param(right_is_double, types::F64);
        ic.append_block_param(right_is_double, types::I64);
        ic.append_block_param(slowpath, types::I64);
        ic.append_block_param(slowpath, types::I64);
        ic.append_block_param(slowpath, types::I64);
        ic.append_block_param(finish, types::I64);
        ic.append_block_param(left_not_int, types::I64);
        ic.append_block_param(left_not_int, types::I64);
        ic.append_block_param(right_not_int, types::I64);
        ic.append_block_param(right_not_int, types::I64);
        ic.append_block_param(fop, types::F64);
        ic.append_block_param(fop, types::F64);

        let is_i = ic.is_int32(left);
        ic.ins().brz(is_i, left_not_int, &[left, right]);
        let next = ic.create_block();
        ic.ins().jump(next, &[]);
        ic.switch_to_block(next);
        let is_i = ic.is_int32(right);
        ic.ins().brz(is_i, right_not_int, &[left, right]);
        let next = ic.create_block();
        ic.ins().jump(next, &[]);
        ic.switch_to_block(next);
        let ileft = ic.as_int32(left);
        let iright = ic.as_int32(right);
        let (result, flags) = ic.ins().iadd_ifcout(ileft, iright);
        ic.ins()
            .brif(IntCC::Overflow, flags, slowpath, &[frame, left, right]);
        ic.fall(&[types::I32], &[result]);
        let bb = ic.current_block().unwrap();
        let p = ic.block_params(bb)[0];
        let res = ic.new_int(p);
        ic.ins().jump(finish, &[res]);

        ic.switch_to_block(left_not_int);
        {
            let left = ic.block_params(left_not_int)[0];
            let right = ic.block_params(left_not_int)[1];
            let left_not_number = ic.is_number(left);
            let right_not_number = ic.is_number(right);
            ic.ins()
                .brz(left_not_number, slowpath, &[frame, left, right]);
            let next = ic.create_block();
            ic.ins().jump(next, &[]);
            ic.switch_to_block(next);
            ic.ins()
                .brz(right_not_number, slowpath, &[frame, left, right]);
            let next = ic.create_block();
            ic.ins().jump(next, &[]);
            ic.switch_to_block(next);
            let res = ic.as_double(left);
            let is_int32 = ic.is_int32(right);
            ic.ins().brz(is_int32, right_is_double, &[res, right]);
            let next = ic.create_block();
            ic.ins().jump(next, &[]);
            ic.switch_to_block(next);
            let as_i = ic.ins().ireduce(types::I32, right);
            let as_d = ic.ins().fcvt_from_sint(types::F64, as_i);
            ic.ins().jump(right_was_integer, &[res, as_d]);
            ic.switch_to_block(right_not_int);
            let left = ic.block_params(right_not_int)[0];
            let right = ic.block_params(right_not_int)[1];
            let right_not_number = ic.is_number(right);
            ic.ins()
                .brz(right_not_number, slowpath, &[frame, left, right]);
            let next = ic.create_block();
            ic.ins().jump(next, &[]);
            ic.switch_to_block(next);
            let as_i = ic.ins().ireduce(types::I32, left);
            let as_d = ic.ins().fcvt_from_sint(types::F64, as_i);

            ic.ins().jump(right_is_double, &[as_d, right]);
            ic.switch_to_block(right_is_double);
            let left = ic.block_params(right_is_double)[0];
            let right = ic.block_params(right_is_double)[1];
            let as_d = ic.as_double(right);
            ic.ins().jump(fop, &[left, as_d]);
        }
        ic.switch_to_block(right_was_integer);
        let left = ic.block_params(right_was_integer)[0];
        let right = ic.block_params(right_was_integer)[1];
        ic.ins().jump(fop, &[left, right]);
        ic.switch_to_block(fop);
        let left = ic.block_params(fop)[0];
        let right = ic.block_params(fop)[1];
        let res = ic.ins().fadd(left, right);
        let boxed = ic.new_double(res);
        ic.ins().jump(finish, &[boxed]);

        ic.switch_to_block(slowpath);
        let v = ic.undefined_value();
        ic.ins().jump(finish, &[v]);
        ic.switch_to_block(finish);
        let res = ic.block_params(finish)[0];
        res
    }
}
