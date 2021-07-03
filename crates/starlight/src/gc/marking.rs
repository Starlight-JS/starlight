use std::mem::{size_of, swap};
use std::ptr::null_mut;

use super::cell::*;
use super::{allocation::Space, cell::GcPointerBase, Address};
use crossbeam::queue::SegQueue;
use crossbeam::sync::Parker;

pub const MARKING_RUNNING: u8 = 0;
pub const MARKING_STOPPED: u8 = 1;
pub const MARKING_TERMINATE: u8 = 2;
pub struct MarkingThread {
    pub incoming: SegQueue<Address>,
    pub worklist: Vec<*mut GcPointerBase>,
    // super unsafe reference to space. We use it to properly mark objects.
    pub space: &'static Space,
    pub state: *mut u8,
    pub p: Parker,
}

impl MarkingThread {
    fn pop(&mut self) -> *mut GcPointerBase {
        if let Some(ptr) = self.worklist.pop() {
            return ptr;
        }
        // if worklist is empty check out objects from remembered set.

        match self.empty_incoming() {
            true => self.pop(),
            false => null_mut(),
        }
    }
    #[cold]
    fn empty_incoming(&mut self) -> bool {
        let mut empty = true;
        while let Some(ptr) = self.incoming.pop() {
            empty = false;
            self.worklist.push(ptr.to_mut_ptr());
        }
        empty
    }
    pub unsafe fn process(&mut self) {
        loop {
            loop {
                let ptr = self.pop();
                if ptr.is_null() {
                    break;
                }

                (*ptr).get_dyn().trace(self);
                (*ptr).set_state(POSSIBLY_GREY, POSSIBLY_BLACK);
            }

            *self.state = MARKING_STOPPED;
            self.p.park();
            if self.state.read() == MARKING_TERMINATE {
                return;
            }
            assert_eq!(self.state.read(), MARKING_RUNNING);
        }
    }
}

impl Tracer for MarkingThread {
    fn visit_weak(&mut self, slot: *const WeakSlot) {
        unsafe {
            let inner = &mut *(slot as *mut WeakSlot);
            inner.state = WeakState::Mark;
        }
    }

    fn visit_raw(&mut self, cell: *mut GcPointerBase) {
        let base = cell;
        unsafe {
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return;
            }
            self.space.mark(cell);
            self.worklist.push(base as *mut _);
        }
    }

    fn visit(&mut self, cell: GcPointer<dyn GcCell>) {
        unsafe {
            let base = cell.base.as_ptr();
            if !(*base).set_state(DEFINETELY_WHITE, POSSIBLY_GREY) {
                return;
            }
            self.space.mark(cell.base.as_ptr());
            self.worklist.push(base);
        }
    }

    fn add_conservative(&mut self, from: usize, to: usize) {
        let mut scan = from;
        let mut end = to;
        if scan > end {
            swap(&mut scan, &mut end);
        }
        unsafe {
            while scan < end {
                let ptr = (scan as *mut *mut u8).read();

                if (*self.space).is_heap_pointer(ptr) {
                    let mut ptr = ptr.cast::<GcPointerBase>();
                    self.visit_raw(ptr);
                    scan += size_of::<usize>();
                    continue;
                }

                scan += size_of::<usize>();
            }
        }
    }
}
