use crate::{
    cryptographically_random_number::cryptographically_random_number, weak_random::WeakRandom,
};

pub fn random_number() -> f64 {
    let bits = cryptographically_random_number() as f64;
    bits / (u32::MAX as f64 + 1.0)
}

pub fn weak_random_u32() -> u32 {
    static mut WEAK_RANDOM: WeakRandom = WeakRandom::const_new();
    unsafe { WEAK_RANDOM.get_u32() }
}
