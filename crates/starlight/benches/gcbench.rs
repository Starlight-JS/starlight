#![allow(dead_code)]
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use starlight::{
    heap::{
        cell::{GcCell, GcPointer, Trace, Tracer},
        snapshot::serializer::{Serializable, SnapshotSerializer},
        Heap,
    },
    vm::{array_storage::ArrayStorage, value::JsValue, GcParams, Runtime, RuntimeParams},
    vtable_impl, Platform,
};
use wtf_rs::keep_on_stack;

pub fn criterion_benchmark(c: &mut Criterion) {
    Platform::initialize();
    let mut rrt = Runtime::new(
        RuntimeParams::default(),
        GcParams::default()
            .with_parallel_marking(true)
            .with_marker_threads(4),
        None,
    );
    let mut rt = rrt.heap();
    let mut _temp_tree = Some(make_tree(&mut rt, STRETCH_TREE_DEPTH as i32));
    _temp_tree = None;
    let mut long_lived = rt.allocate(Node::new(None, None));
    long_lived.j = 0xdead;
    long_lived.i = 0xdead;
    keep_on_stack!(&long_lived);
    populate(&mut rt, LONG_LIVED_TREE_DEPTH as _, long_lived);
    let arr = ArrayStorage::with_size(&mut rrt, ARRAY_SIZE as _, ARRAY_SIZE as _);
    let mut rt = rrt.heap();
    let mut array = rt.allocate(arr);
    for i in 0..(ARRAY_SIZE / 2) {
        *array.at_mut(i as _) = JsValue::encode_f64_value(1.0 / i as f64);
    }
    keep_on_stack!(&mut array);
    let mut depth = MIN_TREE_DEPTH;
    let mut c = c.benchmark_group("gcbench");

    while depth <= MAX_TREE_DEPTH {
        c.sample_size(10).bench_function(
            &format!("Top down construction (depth={})", depth),
            |b| {
                /*b.iter_batched(, routine, size)|| {

                        let mut temp_tree = rt.allocate(Node::new(None, None));
                        keep_on_stack!(&mut temp_tree);
                        populate(&mut rt, depth as _, temp_tree);
                    }

                });*/

                b.iter_batched(
                    || {},
                    |_data| {
                        let mut temp_tree = rt.allocate(Node::new(None, None));
                        keep_on_stack!(&mut temp_tree);
                        populate(&mut rt, depth as _, temp_tree);
                    },
                    BatchSize::NumIterations(num_iters(depth) as _),
                )
            },
        );

        c.sample_size(10).bench_function(
            &format!("Bottom up construction (depth={})", depth),
            |b| {
                b.iter_batched(
                    || {},
                    |_data| {
                        let temp_tree = make_tree(&mut rt, depth as _);
                        keep_on_stack!(&temp_tree);
                    },
                    BatchSize::NumIterations(num_iters(depth) as _),
                );
            },
        );

        depth += 2;
    }

    if long_lived.j != 0xdead || array.at(1000).get_number() != 1.0 / 1000.0 {
        println!(
            "Failed (j = {}, array[1000] = {})",
            long_lived.j,
            array.at(1000).get_number()
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

pub struct Node {
    left: Option<GcPointer<Self>>,
    right: Option<GcPointer<Self>>,
    i: i32,
    j: i32,
}
const STRETCH_TREE_DEPTH: usize = 18;
const LONG_LIVED_TREE_DEPTH: usize = 16;
const ARRAY_SIZE: usize = 500000;
const MIN_TREE_DEPTH: usize = 4;
const MAX_TREE_DEPTH: usize = 16;

const fn tree_size(i: usize) -> usize {
    (1 << (i + 1)) - 1
}

const fn num_iters(i: usize) -> usize {
    2 * tree_size(STRETCH_TREE_DEPTH) / tree_size(i)
}

fn populate(heap: &mut Heap, mut idepth: i32, mut this_node: GcPointer<Node>) {
    keep_on_stack!(&mut this_node);
    if idepth <= 0 {
        return;
    }
    idepth -= 1;
    this_node.left = Some(heap.allocate(Node::new(None, None)));
    this_node.right = Some(heap.allocate(Node::new(None, None)));
    populate(heap, idepth, this_node.left.unwrap());
    populate(heap, idepth, this_node.right.unwrap());
}

fn make_tree(heap: &mut Heap, idepth: i32) -> GcPointer<Node> {
    if idepth <= 0 {
        return heap.allocate(Node::new(None, None));
    }

    let n1 = make_tree(heap, idepth - 1);
    keep_on_stack!(&n1);
    let n2 = make_tree(heap, idepth - 1);
    keep_on_stack!(&n2);
    heap.allocate(Node::new(Some(n1), Some(n2)))
}

impl Node {
    pub fn new(left: Option<GcPointer<Self>>, right: Option<GcPointer<Self>>) -> Self {
        Self {
            left,
            right,
            i: 0,
            j: 0,
        }
    }
}
impl GcCell for Node {
    fn deser_pair(&self) -> (usize, usize) {
        (0, 0)
    }
    vtable_impl!();
}
impl Serializable for Node {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {}
}
unsafe impl Trace for Node {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.left.trace(visitor);
        self.right.trace(visitor);
    }
}
