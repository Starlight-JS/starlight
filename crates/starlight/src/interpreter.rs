use crate::{
    gc::cell::GcPointer,
    vm::{
        code_block::CodeBlock, stack_alignment::round_local_register_count_for_frame_pointer_offset,
    },
};

pub mod callframe;
pub mod register;
pub mod stack;

pub fn frame_register_count_for(cb: GcPointer<CodeBlock>) -> usize {
    round_local_register_count_for_frame_pointer_offset(cb.num_callee_locals())
}
