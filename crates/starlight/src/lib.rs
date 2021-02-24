use vm::value::Value;

pub mod heap;
pub mod utils;
pub mod vm;

pub fn val_add(x: Value, y: Value) -> Value {
    #[cold]
    fn add_slow(x: f64, y: f64) -> Value {
        Value::encode_untrusted_f64_value(x + y)
    }
    if x.is_int32() && y.is_int32() {
        if let Some(x) = x.get_int32().checked_add(y.get_int32()) {
            return Value::encode_int32(x);
        }
    }

    let x = if x.is_int32() {
        x.get_int32() as f64
    } else {
        x.get_double()
    };
    let y = if y.is_int32() {
        y.get_int32() as f64
    } else {
        y.get_double()
    };

    add_slow(x, y)
}
