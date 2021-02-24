use starlight::{
    heap::{
        cell::{GcCell, Trace},
        Heap, SlotVisitor,
    },
    vm::value::JSValue,
};
use wtf_rs::keep_on_stack;

struct Foo {}
unsafe impl Trace for Foo {
    fn trace(&self, _visitor: &mut SlotVisitor) {
        println!("trace foo");
    }
}
impl GcCell for Foo {}
fn main() {
    let mut heap = Heap::new();

    let mut f = JSValue::encode_object_value(heap.allocate(Foo {}).as_dyn());
    println!("Foo at {:x}<-{:p}", f.get_raw(), &f);
    keep_on_stack!(&mut f);
    heap.gc();
}
