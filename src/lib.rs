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
pub mod runtime;
