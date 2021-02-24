use starlight::heap::{
    cell::{GcCell, Trace},
    Heap, SlotVisitor,
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

    let mut f = heap.allocate(Foo {});
    println!("Foo at {:p}<-{:p}", f, &f);
    keep_on_stack!(&mut f);
    heap.gc();
}
