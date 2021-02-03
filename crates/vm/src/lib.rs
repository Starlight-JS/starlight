#![allow(
    clippy::single_match,
    clippy::new_without_default,
    clippy::collapsible_if,
    clippy::float_cmp,
    clippy::eq_op,
    clippy::unnecessary_unwrap,
    clippy::redundant_pattern_matching,
    clippy::needless_lifetimes
)]

/*#![no_std]
extern crate alloc;
 */
#[macro_export]
macro_rules! log_if {
    ($val: expr,$($rest:tt)*) => {
        if $val {
            eprintln!($($rest)*);
        }
    };
}
#[macro_use]
pub mod heap;
pub mod gc;
pub mod runtime;
pub mod util;
