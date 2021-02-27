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
    let s = Structure::new_indexed(&mut rt, None, false);
    let mut obj = JsObject::new(&mut rt, s, JsObject::get_class(), ObjectTag::Ordinary);
    let _ = obj.put(
        &mut rt,
        "x".intern(),
        JsValue::encode_f64_value(42.544),
        false,
    );

    obj.delete(&mut rt, "x".intern(), false)
        .unwrap_or_else(|_| panic!());

    let val = obj.get(&mut rt, "x".intern()).unwrap_or_else(|_| panic!());

    println!("{}", val.is_undefined());
}
