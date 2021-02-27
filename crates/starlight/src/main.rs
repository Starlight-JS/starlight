use starlight::{
    vm::{
        object::{JsObject, ObjectTag},
        structure::Structure,
        symbol_table::Internable,
        value::JsValue,
        Runtime,
    },
    Platform,
};

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(false);
    println!("{}", rt.heap().allocated());
    rt.heap().gc();
    println!("{}", rt.heap().allocated());
}
