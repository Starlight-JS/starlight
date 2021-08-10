use comet::mmap::Mmap;
use std::ptr::null_mut;

use crate::vm::VirtualMachineRef;

use super::{callframe::CallFrame, register::Register};

/// Allow 8k of excess registers before we start trying to reap the stack
pub const MAX_EXCESS_CAPACITY: usize = 8 * 1024;
pub struct InterpreterLoopStack {
    top_call_frame: &'static mut *mut CallFrame,
    end: *mut Register,
    commit_top: *mut Register,
    reservation: Mmap,
    last_stack_pointer: *mut u8,
    current_stack_pointer: *mut u8,
    soft_reserved_zone_in_registers: usize,
    vm: VirtualMachineRef,
}

impl InterpreterLoopStack {
    fn low_address(&self) -> *mut Register {
        self.end
    }

    fn high_address(&self) -> *mut Register {
        (self.reservation.start() as usize + self.reservation.size()) as *mut _
    }

    fn reservation_top(&self) -> *mut Register {
        self.reservation.start() as _
    }

    pub fn size(&self) -> usize {
        self.high_address() as usize - self.low_address() as usize
    }

    pub fn contains_address(&self, addr: *mut Register) -> bool {
        self.low_address() <= addr && addr < self.high_address()
    }
    #[inline]
    pub fn current_stack_pointer(&self) -> *mut u8 {
        self.current_stack_pointer
    }
    pub fn set_loop_stack_limit(&mut self, new_top_of_stack: *mut Register) {
        self.end = new_top_of_stack;
    }

    pub fn new(mut vm: VirtualMachineRef) -> Self {
        let reservation = Mmap::new(4 * 1024 * 1024);

        let mut this = Self {
            vm,
            top_call_frame: unsafe { std::mem::transmute(&mut vm.top_call_frame) },
            reservation,
            end: null_mut(),
            soft_reserved_zone_in_registers: 0,
            last_stack_pointer: null_mut(),
            current_stack_pointer: null_mut(),
            commit_top: null_mut(),
        };
        let bottom_of_stack = this.high_address();
        this.set_loop_stack_limit(bottom_of_stack);
        this.commit_top = bottom_of_stack;
        this.last_stack_pointer = bottom_of_stack.cast();
        this.current_stack_pointer = bottom_of_stack.cast();
        *this.top_call_frame = null_mut();
        this
    }

    pub fn grow(&mut self, mut new_top_of_stack: *mut Register) -> bool {
        unsafe {
            let mut new_top_of_stack_with_reserved_zone =
                new_top_of_stack.sub(self.soft_reserved_zone_in_registers);
            if new_top_of_stack_with_reserved_zone >= self.commit_top {
                self.set_loop_stack_limit(new_top_of_stack);
                return true;
            }

            let mut delta = self.commit_top as usize - new_top_of_stack_with_reserved_zone as usize;
            delta = wtf_rs::round_up_to_multiple_of(16 * 1024, delta);
            let new_commit_top = self.commit_top as usize - (delta / 8);
            if new_commit_top < self.reservation_top() as usize {
                return false;
            }
            self.reservation.commit(new_commit_top as _, delta);
            self.commit_top = new_commit_top as _;
            new_top_of_stack = self.commit_top.add(self.soft_reserved_zone_in_registers);
            self.set_loop_stack_limit(new_top_of_stack);
            true
        }
    }

    pub fn release_excess_capacity(&mut self) {
        unsafe {
            let high_address_with_reserved_zone = self
                .high_address()
                .sub(self.soft_reserved_zone_in_registers);
            let delta = high_address_with_reserved_zone as usize - self.commit_top as usize;
            self.reservation.dontneed(self.commit_top as _, delta);
            self.commit_top = high_address_with_reserved_zone;
        }
    }

    pub fn set_soft_reserved_zone_size(&mut self, reserved_zone_size: usize) {
        self.soft_reserved_zone_in_registers = reserved_zone_size / 8;
        if self.commit_top as usize
            > unsafe { self.end.sub(self.soft_reserved_zone_in_registers) as usize }
        {
            self.grow(self.end);
        }
    }

    pub fn is_safe_to_recurse(&self) -> bool {
        unsafe {
            let reservation_limit = self
                .reservation_top()
                .add(self.soft_reserved_zone_in_registers);
            return self.top_call_frame.is_null()
                || ((**self.top_call_frame).top_of_frame() > reservation_limit as _);
        }
    }
}
