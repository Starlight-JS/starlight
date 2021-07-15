#![allow(dead_code, clippy::float_cmp)]
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use starlight::letroot;
use starlight::prelude::Options;
use starlight::vm::context::Context;
use starlight::{
    gc::{
        cell::{GcCell, GcPointer, Trace, Tracer},
        snapshot::serializer::{Serializable, SnapshotSerializer},
    },
    vm::{array_storage::ArrayStorage, value::JsValue, Runtime},
    Platform,
};
use wtf_rs::keep_on_stack;
pub fn criterion_benchmark(c: &mut Criterion) {
    Platform::initialize();
    let rrt = Runtime::new(Options::default(), None);
    let stack = rrt.shadowstack();
    let mut rt = rrt;
    let ctx = Context::new(&mut rt);

    let mut _temp_tree = Some(make_tree(&mut rt, STRETCH_TREE_DEPTH as i32));
    _temp_tree = None;
    letroot!(
        long_lived = stack,
        rt.heap().allocate(Node::new(None, None))
    );
    long_lived.j = 0xdead;
    long_lived.i = 0xdead;
    keep_on_stack!(&long_lived);
    populate(&mut rt, LONG_LIVED_TREE_DEPTH as _, &mut long_lived);
    let arr = ArrayStorage::with_size(ctx, ARRAY_SIZE as _, ARRAY_SIZE as _);

    letroot!(array = stack, arr);
    for i in 0..(ARRAY_SIZE / 2) {
        *array.at_mut(i as _) = JsValue::new(1.0 / i as f64);
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
                        letroot!(temp_tree = stack, rt.heap().allocate(Node::new(None, None)));
                        keep_on_stack!(&mut temp_tree);
                        populate(&mut rt, depth as _, &mut temp_tree);
                        rt.heap().collect_if_necessary();
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
                        letroot!(temp_tree = stack, make_tree(&mut rt, depth as _));
                        keep_on_stack!(&temp_tree);
                        rt.heap().collect_if_necessary();
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

fn populate(gc: &mut Runtime, mut idepth: i32, this_node: &mut GcPointer<Node>) {
    gc.heap().collect_if_necessary();

    if idepth <= 0 {
        return;
    }
    idepth -= 1;
    this_node.left = Some(gc.heap().allocate(Node::new(None, None)));
    this_node.right = Some(gc.heap().allocate(Node::new(None, None)));

    populate(gc, idepth, this_node.left.as_mut().unwrap());
    populate(gc, idepth, this_node.right.as_mut().unwrap());
}

fn make_tree(gc: &mut Runtime, idepth: i32) -> GcPointer<Node> {
    if idepth <= 0 {
        return gc.heap().allocate(Node::new(None, None));
    }
    let stack = gc.shadowstack();

    letroot!(n1 = stack, make_tree(gc, idepth - 1));
    letroot!(n2 = stack, make_tree(gc, idepth - 1));
    gc.heap().collect_if_necessary();
    gc.heap().allocate(Node::new(Some(*n1), Some(*n2)))
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
