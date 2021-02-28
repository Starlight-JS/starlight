use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use gc::{Gc, GcCell};
use gc_derive::Trace;
use starlight::vm::value::JsValue;
use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
pub fn criterion_benchmark(c: &mut Criterion) {
    let mut _temp_tree = Some(make_tree(STRETCH_TREE_DEPTH as i32));
    _temp_tree = None;
    let mut long_lived = Gc::new(GcCell::new(Node::new(None, None)));
    long_lived.borrow_mut().j = 0xdead;
    long_lived.borrow_mut().i = 0xdead;

    populate(LONG_LIVED_TREE_DEPTH as _, long_lived.clone());

    let mut depth = MIN_TREE_DEPTH;
    let mut c = c.benchmark_group("gcbench-rc");

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
                        let temp_tree = Gc::new(GcCell::new(Node::new(None, None)));

                        populate(depth as _, temp_tree.clone());
                        drop(temp_tree);
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
                        let temp_tree = black_box(make_tree(depth as _));
                        drop(temp_tree);
                    },
                    BatchSize::NumIterations(num_iters(depth) as _),
                );
            },
        );

        depth += 2;
    }

    if long_lived.borrow().j != 0xdead {
        println!("Failed");
    }
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

#[derive(Trace)]
pub struct Node {
    left: Option<Gc<GcCell<Self>>>,
    right: Option<Gc<GcCell<Self>>>,
    i: i32,
    j: i32,
}

impl gc::Finalize for Node {}
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

fn populate(mut idepth: i32, mut this_node: Gc<GcCell<Node>>) {
    //keep_on_stack!(&mut this_node);
    if idepth <= 0 {
        return;
    }
    idepth -= 1;
    this_node.borrow_mut().left = Some(Gc::new(GcCell::new(Node::new(None, None))));
    this_node.borrow_mut().right = Some(Gc::new(GcCell::new(Node::new(None, None))));
    populate(idepth, this_node.borrow().left.clone().unwrap());
    populate(idepth, this_node.borrow().right.clone().unwrap());
}

fn make_tree(idepth: i32) -> Gc<GcCell<Node>> {
    if idepth <= 0 {
        return Gc::new(GcCell::new(Node::new(None, None)));
    }

    let n1 = make_tree(idepth - 1);
    //keep_on_stack!(&n1);
    let n2 = make_tree(idepth - 1);
    //keep_on_stack!(&n2);
    Gc::new(GcCell::new(Node::new(Some(n1), Some(n2))))
}

impl Node {
    pub fn new(left: Option<Gc<GcCell<Self>>>, right: Option<Gc<GcCell<Self>>>) -> Self {
        Self {
            left,
            right,
            i: 0,
            j: 0,
        }
    }
}
