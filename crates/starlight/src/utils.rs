pub mod align_as;
pub mod ordered_set;
#[macro_export]
macro_rules! log_if {
    ($cond: expr, $($fmt:tt)*) => {
        if $cond {
            print!($($fmt)*);
        }
    };
}

#[macro_export]
macro_rules! logln_if {
    ($cond: expr, $($fmt:tt)*) => {
        if $cond {
            println!($($fmt)*);
        }
    };
}
