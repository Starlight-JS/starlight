#![allow(dead_code)]
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use starlight::vm::value::JsValue;
use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
struct Base<T> {
    rc: u32,
    value: T,
}
pub struct Rc<T> {
    value: NonNull<Base<T>>,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(Base { value, rc: 1 })))
            },
        }
    }
    fn base(&self) -> &mut Base<T> {
        unsafe { &mut *self.value.as_ptr() }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        self.base().rc += 1;
        Self { value: self.value }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        self.base().rc -= 1;
        if self.base().rc == 0 {
            unsafe {
                let _ = Box::from_raw(self.value.as_ptr());
            }
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.base().value
    }
}
impl<T> DerefMut for Rc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base().value
    }
}
pub fn criterion_benchmark(c: &mut Criterion) {
    let mut _temp_tree = Some(make_tree(STRETCH_TREE_DEPTH as i32));
    _temp_tree = None;
    let mut long_lived = Rc::new(Node::new(None, None));
    long_lived.j = 0xdead;
    long_lived.i = 0xdead;

    populate(LONG_LIVED_TREE_DEPTH as _, long_lived.clone());

    let mut array = Rc::new([JsValue::new(0.0); ARRAY_SIZE]);
    for i in 0..(ARRAY_SIZE / 2) {
        array[i] = JsValue::new(1.0 / i as f64);
    }

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
                        let temp_tree = Rc::new(Node::new(None, None));

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

    if long_lived.j != 0xdead || array[1000].get_number() != 1.0 / 1000.0 {
        println!("Failed");
    }
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

pub struct Node {
    left: Option<Rc<Self>>,
    right: Option<Rc<Self>>,
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

fn populate(mut idepth: i32, mut this_node: Rc<Node>) {
    //keep_on_stack!(&mut this_node);
    if idepth <= 0 {
        return;
    }
    idepth -= 1;
    this_node.left = Some(Rc::new(Node::new(None, None)));
    this_node.right = Some(Rc::new(Node::new(None, None)));
    populate(idepth, this_node.left.clone().unwrap());
    populate(idepth, this_node.right.clone().unwrap());
}

fn make_tree(idepth: i32) -> Rc<Node> {
    if idepth <= 0 {
        return Rc::new(Node::new(None, None));
    }

    let n1 = make_tree(idepth - 1);
    //keep_on_stack!(&n1);
    let n2 = make_tree(idepth - 1);
    //keep_on_stack!(&n2);
    Rc::new(Node::new(Some(n1), Some(n2)))
}

impl Node {
    pub fn new(left: Option<Rc<Self>>, right: Option<Rc<Self>>) -> Self {
        Self {
            left,
            right,
            i: 0,
            j: 0,
        }
    }
}
