/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
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
