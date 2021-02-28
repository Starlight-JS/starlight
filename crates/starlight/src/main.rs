use starlight::{
    bytecode::profile::ArithProfile,
    vm::{value::JsValue, Runtime},
    Platform,
};

fn main() {
    Platform::initialize();
    let _rt = Runtime::new(false);
    let mut profile = ArithProfile::Binary(0);

    let x = JsValue::encode_f64_value(42.2);
    let y = JsValue::encode_f64_value(3.4);
    let z = x.get_number() + y.get_number();
    profile.observe_lhs_and_rhs(x, y);
    profile.observe_result(JsValue::encode_f64_value(z));
    println!("{}", profile.lhs_observed_type().is_only_number());
    println!("{}", profile.observed_results().did_observe_double());
    println!("{}", profile.rhs_observed_type().is_only_number());
}
