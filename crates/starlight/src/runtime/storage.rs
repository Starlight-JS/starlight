use super::gc_array::GcVec;
use crate::gc::cell::*;
use crate::vm::VirtualMachine;
use std::ops::{Index, IndexMut};

pub struct FixedStorage<T: Cell + Copy> {
    pub(crate) data: GcVec<T>,
}

impl<T: Cell + Copy + Default> FixedStorage<T> {
    pub fn reserve(&mut self, vm: &mut VirtualMachine, n: usize) {
        /*if n > self.capacity() {
            let next = if n == 0 {
                0
            } else if n < 8 {
                8
            } else {
                clp2(n)
            };
            let ptr = GcArray::<T>::new(vm.space(), next, T::default());
            unsafe {
                std::ptr::copy_nonoverlapping(self.data.begin(), ptr.begin(), self.data.len());
            }

            self.data = ptr;
        }*/
        self.data.reserve(vm, n);
    }
    pub fn resize(&mut self, vm: &mut VirtualMachine, n: usize, value: T) {
        /*let previous = self.capacity();
        self.reserve(vm, n);
        if previous < self.capacity() {
            for i in previous..self.capacity() {
                self.data[i] = value;
            }
        }*/
        self.data.resize(vm, n, value);
    }

    pub fn size(&self) -> usize {
        self.capacity()
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    pub fn new(vm: &mut VirtualMachine, init: T) -> Self {
        Self {
            data: GcVec::new(vm, 0),
        }
    }

    pub fn with_capacity(vm: &mut VirtualMachine, cap: usize, init: T) -> Self {
        if cap == 0 {
            return Self {
                data: GcVec::new(vm, cap),
            };
        }
        let mut this = Self {
            data: GcVec::new(vm, cap),
        };

        for _ in 0..cap {
            this.data.push(vm, init);
        }

        this
    }

    pub fn with(vm: &mut VirtualMachine, cap: usize, value: T) -> Self {
        let mut this = Self::new(vm, T::default());
        this.resize(vm, cap, value);
        this
    }
}

impl<T: Cell + Copy> Index<usize> for FixedStorage<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T: Cell + Copy> IndexMut<usize> for FixedStorage<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl<T: Cell + Copy> Cell for FixedStorage<T> {}

unsafe impl<T: Cell + Copy> Trace for FixedStorage<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        /*for i in 0..self.data.len() {
            self.data[i].trace(tracer);
        }*/

        self.data.trace(tracer); //self.data.iter().for_each(|x| x.trace(tracer));
    }
}
