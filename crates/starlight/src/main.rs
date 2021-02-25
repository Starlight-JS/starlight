use starlight::{
    heap::usable_size,
    vm::{array_storage::ArrayStorage, value::JsValue, Runtime},
    Platform,
};
use wtf_rs::keep_on_stack;

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(true);
    let mut arr = ArrayStorage::new(&mut rt, 0);
    arr.push_back(&mut rt, JsValue::encode_f64_value(42.42));
    assert!(arr.pop_back(&mut rt).get_double() == 42.42);
    rt.heap().gc();
    println!("{:p}->{:p}", &arr, arr);
    println!("{}", rt.heap().allocation_track(arr));
}
