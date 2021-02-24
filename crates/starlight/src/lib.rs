use vm::value::JSValue;

pub mod heap;
pub mod utils;
pub mod vm;

pub fn val_add(x: JSValue, y: JSValue, slowpath: fn(JSValue, JSValue) -> JSValue) -> JSValue {
    if x.is_double() && y.is_double() {
        return JSValue::encode_f64_value(x.get_double() + y.get_double());
    }

    slowpath(x, y)
}
