use std::fmt;

use crate::conventions::FIRST_CONSTANT_REGISTER_INDEX;

pub const fn virtual_register_is_local(op: i32) -> bool {
    op < 0
}

pub const fn virtual_register_is_argument(op: i32) -> bool {
    op >= 0
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualRegister(pub i32);

impl VirtualRegister {
    pub const fn new(virtual_register: i32) -> Self {
        Self(virtual_register)
    }

    pub const fn is_local(self) -> bool {
        virtual_register_is_local(self.0)
    }
    pub const fn is_argument(self) -> bool {
        virtual_register_is_argument(self.0)
    }
    pub const fn is_constant(self) -> bool {
        self.0 >= FIRST_CONSTANT_REGISTER_INDEX
    }
    pub const fn to_local(self) -> i32 {
        operand_to_local(self.0)
    }

    pub const fn to_argument(self) -> i32 {
        operand_to_argument(self.0)
    }
    pub const fn to_constant_index(self) -> i32 {
        self.0 - FIRST_CONSTANT_REGISTER_INDEX
    }
    pub const fn offset(self) -> i32 {
        self.0
    }
    pub const fn offset_in_bytes(self) -> i32 {
        self.0 * 8
    }

    pub const fn for_local(x: i32) -> Self {
        Self(local_to_operand(x))
    }

    pub const fn for_argument(x: i32) -> Self {
        Self(argument_to_operand(x))
    }
}

const fn operand_to_local(op: i32) -> i32 {
    -1 - op
}
const fn local_to_operand(op: i32) -> i32 {
    -1 - op
}

// TODO: Callframe this offset
const fn operand_to_argument(op: i32) -> i32 {
    op
}

const fn argument_to_operand(op: i32) -> i32 {
    op
}
