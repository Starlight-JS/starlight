use std::mem::{replace, take};

use crate::gc::cell::{POSSIBLY_BLACK, POSSIBLY_GREY};

use super::{
    cell::{GcPointerBase, Tracer},
    Heap, SlotVisitor,
};

pub struct IncrementalMarking {
    budget: isize,
    steps: usize,
    state: IncrementalMarkingState,
    queue: Vec<*mut GcPointerBase>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IncrementalMarkingState {
    MarkRoots,
    Marking,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IncrementalMarkingResult {
    RanOutOfSteps,
    RanOutOfBudget,
    Complete,
    Continue,
}
#[inline(never)]
fn current_sp() -> usize {
    let mut sp: usize = 0;
    sp = &sp as *const usize as usize;
    sp
}
impl IncrementalMarking {
    pub fn new() -> Self {
        Self {
            budget: 0,
            steps: 0,
            state: IncrementalMarkingState::MarkRoots,
            queue: Vec::new(),
        }
    }

    pub unsafe fn step(
        &mut self,
        heap: &mut Heap,
        visitor: &mut SlotVisitor,
    ) -> IncrementalMarkingResult {
        match self.state {
            IncrementalMarkingState::MarkRoots => {
                crate::vm::thread::THREAD.with(|thread| {
                    visitor.add_conservative(thread.bounds.origin as _, current_sp() as usize);
                });
                let mut constraints = replace(&mut heap.constraints, vec![]);
                for constraint in constraints.iter_mut() {
                    constraint.execute(visitor);
                }
                heap.constraints = constraints;
                self.queue = take(&mut visitor.queue);
                self.state = IncrementalMarkingState::Marking;
                // if during the cycle 45% more objects are allocated we're likely to go to full STW
                self.budget = (heap.n_allocated as f64 * 1.45) as isize;
                // heap.progression is 0.118 by default.
                self.steps = (heap.n_allocated as f64 * heap.progression) as usize;
                return IncrementalMarkingResult::Continue;
            }
            IncrementalMarkingState::Marking => {
                // when we enter mark cycle we want to copy queue to visitor.
                visitor.queue = take(&mut self.queue);
                while let Some(item) = visitor.queue.pop() {
                    if self.steps == 0 {
                        return IncrementalMarkingResult::RanOutOfSteps; // continue mutator execution
                    }
                    if self.budget <= 0 {
                        return IncrementalMarkingResult::RanOutOfBudget; // perform full STW GC
                    }
                    self.steps -= 1;
                    self.budget -= 1;
                    assert!((*item).set_state(POSSIBLY_GREY, POSSIBLY_BLACK));
                    (*item).get_dyn().trace(visitor);
                }

                // Remark roots
                crate::vm::thread::THREAD.with(|thread| {
                    visitor.add_conservative(thread.bounds.origin as _, current_sp() as usize);
                });
                let mut constraints = replace(&mut heap.constraints, vec![]);
                for constraint in constraints.iter_mut() {
                    constraint.execute(visitor);
                }
                heap.constraints = constraints;
                while let Some(item) = visitor.queue.pop() {
                    assert!((*item).set_state(POSSIBLY_GREY, POSSIBLY_BLACK));
                    (*item).get_dyn().trace(visitor);
                }
                self.state = IncrementalMarkingState::MarkRoots;
                return IncrementalMarkingResult::Complete;
            }
        }
    }
}
