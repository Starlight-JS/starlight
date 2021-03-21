use super::{
    cell::{GcPointerBase, POSSIBLY_BLACK, POSSIBLY_GREY},
    SlotVisitor,
};
use crossbeam::deque::{Injector, Steal, Stealer, Worker};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::thread;
use std::time::Duration;
use std::{
    intrinsics::{prefetch_read_data, prefetch_write_data},
    sync::atomic::{AtomicUsize, Ordering},
};
use yastl::Pool;

pub fn start(rootset: &[*mut GcPointerBase], n_workers: usize, threadpool: &mut Pool) {
    let number_workers = n_workers;
    let mut workers = Vec::with_capacity(number_workers);
    let mut stealers = Vec::with_capacity(number_workers);
    let injector = Injector::new();

    for _ in 0..number_workers {
        let w = Worker::new_lifo();
        let s = w.stealer();
        workers.push(w);
        stealers.push(s);
    }

    for root in rootset {
        injector.push(*root as usize);
    }
    let terminator = Terminator::new(number_workers);

    threadpool.scoped(|scoped| {
        for (task_id, worker) in workers.into_iter().enumerate() {
            let injector = &injector;
            let stealers = &stealers;
            let terminator = &terminator;

            scoped.execute(move || {
                let mut task = MarkingTask {
                    task_id,
                    visitor: SlotVisitor {
                        cons_roots: vec![],
                        queue: Vec::with_capacity(256),

                        bytes_visited: 0,
                        sp: 0 as _,
                    },
                    worker,
                    injector,
                    stealers,
                    terminator,

                    marked: 0,
                };

                task.run();
            });
        }
    });
}

type Address = usize;
pub struct Terminator {
    const_nworkers: usize,
    nworkers: AtomicUsize,
}

impl Terminator {
    pub fn new(number_workers: usize) -> Terminator {
        Terminator {
            const_nworkers: number_workers,
            nworkers: AtomicUsize::new(number_workers),
        }
    }

    pub fn try_terminate(&self) -> bool {
        if self.const_nworkers == 1 {
            return true;
        }

        if self.decrease_workers() {
            // reached 0, no need to wait
            return true;
        }

        thread::sleep(Duration::from_micros(1));
        self.zero_or_increase_workers()
    }

    fn decrease_workers(&self) -> bool {
        self.nworkers.fetch_sub(1, Ordering::Relaxed) == 1
    }

    fn zero_or_increase_workers(&self) -> bool {
        let mut nworkers = self.nworkers.load(Ordering::Relaxed);

        loop {
            if nworkers == 0 {
                return true;
            }

            let prev_nworkers = match self.nworkers.compare_exchange(
                nworkers,
                nworkers + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(x) => x,
                Err(x) => x,
            };

            if nworkers == prev_nworkers {
                // Value was successfully increased again, workers didn't terminate in time. There is still work left.
                return false;
            }

            nworkers = prev_nworkers;
        }
    }
}

struct MarkingTask<'a> {
    task_id: usize,

    worker: Worker<Address>,
    injector: &'a Injector<Address>,
    stealers: &'a [Stealer<Address>],
    terminator: &'a Terminator,

    marked: usize,
    visitor: SlotVisitor,
}

impl<'a> MarkingTask<'a> {
    fn pop(&mut self) -> Option<Address> {
        self.pop_local()
            .or_else(|| self.pop_worker())
            .or_else(|| self.pop_global())
            .or_else(|| self.steal())
    }
    /// Pop from local queue

    fn pop_local(&mut self) -> Option<Address> {
        if self.visitor.queue.is_empty() {
            return None;
        }

        let obj = self.visitor.queue.pop().expect("should be non-empty");
        Some(obj as _)
    }
    /// Pop object from current worker queue
    fn pop_worker(&mut self) -> Option<Address> {
        self.worker.pop()
    }
    /// Pop object for marking from global queue
    fn pop_global(&mut self) -> Option<Address> {
        loop {
            let result = self.injector.steal_batch_and_pop(&mut self.worker);

            match result {
                Steal::Empty => break,
                Steal::Success(value) => return Some(value),
                Steal::Retry => continue,
            }
        }

        None
    }
    /// Try to steal object for marking from other marking thread
    fn steal(&self) -> Option<Address> {
        if self.stealers.len() == 1 {
            return None;
        }

        let mut rng = thread_rng();
        let range = Uniform::new(0, self.stealers.len());

        for _ in 0..2 * self.stealers.len() {
            let mut stealer_id = self.task_id;

            while stealer_id == self.task_id {
                stealer_id = range.sample(&mut rng);
            }

            let stealer = &self.stealers[stealer_id];

            loop {
                match stealer.steal_batch_and_pop(&self.worker) {
                    Steal::Empty => break,
                    Steal::Success(address) => return Some(address),
                    Steal::Retry => continue,
                }
            }
        }

        None
    }

    fn run(&mut self) {
        loop {
            let object_addr = if let Some(addr) = self.pop() {
                addr as *mut GcPointerBase
            } else if self.terminator.try_terminate() {
                break;
            } else {
                continue;
            };

            unsafe {
                let object = &mut *object_addr;

                if object.set_state(POSSIBLY_GREY, POSSIBLY_BLACK) {
                    object.get_dyn().trace(&mut self.visitor);

                    // if too many objects is in queue just push some of them to injector
                    // so other marking threads will do some work too.
                    self.maybe_push_to_injector();
                } else {
                    // continue, other thread already marked this object
                    continue;
                }
            }
        }
    }

    fn maybe_push_to_injector(&mut self) {
        if self.visitor.bytes_visited > 100 {
            if self.visitor.queue.len() > 4 {
                let target_len = self.visitor.queue.len() / 2;
                while self.visitor.queue.len() > target_len {
                    let val = self.visitor.queue.pop().unwrap();
                    self.injector.push(val as usize);
                }
            }
            self.visitor.bytes_visited = 0;
        }
    }
}
