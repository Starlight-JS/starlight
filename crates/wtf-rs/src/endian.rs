use crate::swap_byte_order::SwapByteOrder;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Endianess {
    Little,
    Big,
    Native,
}

pub const fn system_endianess() -> Endianess {
    #[cfg(target_endian = "little")]
    {
        Endianess::Little
    }
    #[cfg(target_endian = "big")]
    {
        Endianess::Big
    }
}
pub fn byte_swap<T: SwapByteOrder>(value: T, endian: Endianess) -> T {
    if endian != Endianess::Native && endian != system_endianess() {
        return value.swap_byte_order();
    }
    value
}
