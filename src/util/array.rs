use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::{Slot, Tracer},
    runtime::{
        js_cell::{allocate_cell, JsCell},
        ref_ptr::Ref,
        vm::JsVirtualMachine,
    },
};
use std::{
    mem::size_of,
    ops::{Index, IndexMut},
};

#[repr(C)]
pub struct GcArray<T: HeapObject> {
    len: usize,
    data: [T; 0],
}

impl<T: HeapObject> GcArray<T> {
    pub fn new(vm: Ref<JsVirtualMachine>, len: usize) -> Handle<Self>
    where
        T: Default,
    {
        let val = Self { len: len, data: [] };
        let mut cell = allocate_cell(vm, len * size_of::<T>() + size_of::<GcArray<T>>(), val);
        for i in 0..cell.len() {
            cell[i] = T::default();
        }
        cell
    }
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T: HeapObject> Index<usize> for GcArray<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.data.as_ptr().add(index) }
    }
}
impl<T: HeapObject> IndexMut<usize> for GcArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.data.as_mut_ptr().add(index) }
    }
}
impl<T: HeapObject> HeapObject for GcArray<T> {
    fn needs_destruction(&self) -> bool {
        std::mem::needs_drop::<T>()
    }

    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        for i in 0..self.len() {
            tracer.trace(Slot::new(&mut self[i]));
        }
    }
}

impl<T: HeapObject> JsCell for GcArray<T> {}
