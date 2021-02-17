#![allow(dead_code, unused_mut, unused_variables)]
#![cfg_attr(feature = "valgrind-gc", feature(llvm_asm, backtrace))]
#![allow(
    clippy::transmute_float_to_int,
    clippy::transmute_int_to_float,
    clippy::cast_ref_to_mut,
    clippy::float_cmp,
    clippy::float_arithmetic,
    clippy::identity_op,
    clippy::single_match,
    clippy::new_ret_no_self,
    clippy::needless_range_loop,
    clippy::transmute_ptr_to_ref,
    clippy::new_without_default,
    clippy::should_implement_trait,
    clippy::collapsible_if,
    clippy::len_without_is_empty,
    clippy::eq_op,
    clippy::collapsible_else_if,
    clippy::upper_case_acronyms,
    clippy::unnecessary_wraps,
    clippy::needless_return,
    clippy::single_match,
    unknown_lints,
    unused_unsafe
)]
use runtime::value::JsValue;

pub mod bytecode;
pub mod frontend;
pub mod gc;
pub mod heap;
pub mod interpreter;
pub mod jsrt;
pub mod runtime;
pub mod symbol_table;
pub mod utils;
pub mod vm;

pub fn val_add(x: JsValue, y: JsValue) -> JsValue {
    if x.is_number() && y.is_number() {
        if x.is_int32() && y.is_int32() {
            if let Some(val) = x.as_int32().checked_add(y.as_int32()) {
                return JsValue::new(val);
            }
        }
        let n = if x.is_int32() {
            x.as_int32() as f64
        } else {
            x.as_double()
        };
        let n2 = if y.is_int32() {
            y.as_int32() as f64
        } else {
            y.as_double()
        };
        return JsValue::new(n + n2);
    }
    JsValue::undefined()
}
