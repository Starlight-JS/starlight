use starlight::{
    heap::snapshot::{deserializer, Snapshot},
    vm::{object::JsObject, Runtime},
    Platform,
};
use wtf_rs::keep_on_stack;

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(false, None);
    rt.heap().gc();
    let my_obj = JsObject::new_empty(&mut rt);
    keep_on_stack!(&my_obj);
    println!("{}", rt.heap().allocated());
    let snapshot = Snapshot::take(!true, &mut rt);
    drop(rt);
    std::fs::write("snapshot.out", &snapshot.buffer).unwrap();
    let mut rt = deserializer::Deserializer::deserialize(!true, &snapshot.buffer, None);
    println!("{}", rt.heap().allocated());
}
