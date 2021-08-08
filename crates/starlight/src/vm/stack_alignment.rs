use wtf_rs::round_up_to_multiple_of;

use crate::interpreter::callframe::CallerFrameAndPC;

/// Align local register count to make the last local end on a stack aligned address given the
/// CallFrame is at an address that is stack aligned minus CallerFrameAndPC::sizeInRegisters
pub fn round_local_register_count_for_frame_pointer_offset(local_register_count: u32) -> usize {
    round_up_to_multiple_of(
        2,
        local_register_count as usize + CallerFrameAndPC::SIZE_IN_REGS,
    ) - CallerFrameAndPC::SIZE_IN_REGS
}
