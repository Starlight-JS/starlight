use std::ops::{Index, IndexMut};

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    util::array::GcArray,
};

use super::{ref_ptr::AsRefPtr, vm::JsVirtualMachine};

pub struct FixedStorage<T: HeapObject + Copy> {
    pub(crate) data: Handle<GcArray<T>>,
}
fn clp2(number: usize) -> usize {
    let x = number - 1;
    let x = x | (x >> 1);
    let x = x | (x >> 2);
    let x = x | (x >> 4);
    let x = x | (x >> 8);
    let x = x | (x >> 16);
    x + 1
}
impl<T: HeapObject + Copy + Default> FixedStorage<T> {
    pub fn reserve(&mut self, vm: impl AsRefPtr<JsVirtualMachine>, n: usize) {
        if n > self.capacity() {
            let next = if n == 0 {
                0
            } else if n < 8 {
                8
            } else {
                clp2(n)
            };
            let ptr = GcArray::<T>::new(vm.as_ref_ptr(), next);
            unsafe {
                std::ptr::copy_nonoverlapping(self.data.begin(), ptr.begin(), self.data.len());
            }

            self.data = ptr;
        }
    }
    pub fn resize(&mut self, vm: impl AsRefPtr<JsVirtualMachine>, n: usize, value: T) {
        let previous = self.capacity();
        self.reserve(vm, n);
        if previous < self.capacity() {
            for i in previous..self.capacity() {
                self.data[i] = value;
            }
        }
    }

    pub fn size(&self) -> usize {
        self.capacity()
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    pub fn new(vm: impl AsRefPtr<JsVirtualMachine>) -> Self {
        Self {
            data: GcArray::new(vm.as_ref_ptr(), 0),
        }
    }

    pub fn with_capacity(vm: impl AsRefPtr<JsVirtualMachine>, cap: usize) -> Self {
        Self {
            data: GcArray::new(vm.as_ref_ptr(), cap),
        }
    }

    pub fn with(vm: impl AsRefPtr<JsVirtualMachine>, cap: usize, value: T) -> Self {
        let mut this = Self::new(vm.as_ref_ptr());
        this.resize(vm, cap, value);
        this
    }
}

impl<T: HeapObject + Copy> Index<usize> for FixedStorage<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T: HeapObject + Copy> IndexMut<usize> for FixedStorage<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}
