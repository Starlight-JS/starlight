pub trait SwapByteOrder {
    fn swap_byte_order(self) -> Self;
}

macro_rules! impl_ {
    ($($t: ident)*) => {
        $(
            impl SwapByteOrder for $t {
                fn swap_byte_order(self) -> Self {
                    self.swap_bytes()
                }
            }
        )*
    };
}

impl_! {u8 u16 u32 u64 u128 i8 i16 i32 i64 i128}

impl SwapByteOrder for f32 {
    fn swap_byte_order(self) -> Self {
        f32::from_bits(self.to_bits().swap_byte_order())
    }
}

impl SwapByteOrder for f64 {
    fn swap_byte_order(self) -> Self {
        f64::from_bits(self.to_bits().swap_byte_order())
    }
}

pub fn get_swapped_bytes<T: SwapByteOrder>(value: T) -> T {
    value.swap_byte_order()
}
