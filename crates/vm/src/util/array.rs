use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::{Slot, Tracer},
    runtime::{
        js_cell::{allocate_cell, JsCell},
        vm::JsVirtualMachine,
    },
};
fn clp2(number: usize) -> usize {
    let x = number - 1;
    let x = x | (x >> 1);
    let x = x | (x >> 2);
    let x = x | (x >> 4);
    let x = x | (x >> 8);
    let x = x | (x >> 16);
    x + 1
}
use std::{
    mem::size_of,
    ops::{Index, IndexMut},
    usize,
};

#[repr(C)]
pub struct GcArray<T: HeapObject> {
    len: usize,
    data: [T; 0],
}

impl<T: HeapObject> GcArray<T> {
    pub fn begin(&self) -> *mut T {
        self.data.as_ptr() as *mut _
    }

    pub fn is_empty(&self) -> bool {
        self.len() != 0
    }

    pub fn end(&self) -> *mut T {
        unsafe { self.begin().add(self.len) }
    }
    pub fn new(vm: &mut JsVirtualMachine, len: usize) -> Handle<Self>
    where
        T: Default,
    {
        let val = Self { len, data: [] };
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
#[repr(C)]
struct RawVec<T: HeapObject> {
    cap: usize,
    len: usize,
    data: [T; 0],
}

impl<T: HeapObject> RawVec<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn new(vm: &mut JsVirtualMachine, len: usize) -> Handle<Self> {
        let val = Self {
            len: 0,
            cap: len,
            data: [],
        };
        allocate_cell(vm, len * size_of::<T>() + size_of::<RawVec<T>>(), val)
    }
}

impl<T: HeapObject> HeapObject for RawVec<T> {
    fn needs_destruction(&self) -> bool {
        std::mem::needs_drop::<T>()
    }

    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        for i in 0..self.len() {
            self[i].visit_children(tracer);
        }
    }
    fn compute_size(&self) -> usize {
        size_of::<T>() * self.len + size_of::<Self>()
    }
}

impl<T: HeapObject> JsCell for RawVec<T> {}
impl<T: HeapObject> Index<usize> for RawVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.data.as_ptr().add(index) }
    }
}
impl<T: HeapObject> IndexMut<usize> for RawVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.data.as_mut_ptr().add(index) }
    }
}

pub struct GcVec<T: HeapObject> {
    raw: Handle<RawVec<T>>,
}

impl<T: HeapObject> GcVec<T> {
    pub fn new(vm: &mut JsVirtualMachine, cap: usize) -> Self {
        Self {
            raw: RawVec::new(vm, cap),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() != 0
    }
    pub fn reserve(&mut self, vm: &mut JsVirtualMachine, n: usize) {
        if n > self.raw.cap {
            let next = if n < 8 { 8 } else { clp2(n) };
            let mut ptr = RawVec::<T>::new(vm, next);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.raw.data.as_ptr(),
                    ptr.data.as_mut_ptr(),
                    self.raw.len,
                );
            }
            ptr.len = self.raw.len;

            self.raw = ptr;
        }
    }
    pub fn resize(&mut self, vm: &mut JsVirtualMachine, n: usize, val: T)
    where
        T: Clone,
    {
        let prev = self.raw.len;
        self.reserve(vm, n);
        self.raw.len = n;
        if prev < n {
            for i in prev..n {
                self[i] = val.clone();
            }
        }
    }
    pub fn clear(&mut self) {
        for i in 0..self.len() {
            unsafe {
                let ptr = self.raw.data.as_mut_ptr().add(i);
                core::ptr::drop_in_place(ptr);
            }
        }
        self.raw.len = 0;
    }
    pub fn shrink_to_fit(&mut self, vm: &mut JsVirtualMachine) {
        unsafe {
            let next = self.raw.len;
            let mut ptr = RawVec::<T>::new(vm, next);
            {
                std::ptr::copy_nonoverlapping(
                    self.raw.data.as_ptr(),
                    ptr.data.as_mut_ptr(),
                    self.raw.len,
                );
            }
            ptr.len = self.raw.len;
            self.raw = ptr;
        }
    }

    pub fn insert(&mut self, vm: &mut JsVirtualMachine, index: usize, element: T) {
        #[cold]
        #[inline(never)]
        fn assert_failed(index: usize, len: usize) -> ! {
            panic!(
                "insertion index (is {}) should be <= len (is {})",
                index, len
            );
        }
        let len = self.raw.len;
        if index > self.raw.len {
            assert_failed(index, len);
        }
        if len == self.raw.cap {
            self.reserve(vm, len + 1);
        }
        unsafe {
            {
                let p = self.raw.data.as_mut_ptr().add(index);
                std::ptr::copy(p, p.offset(1), len - index);
                std::ptr::write(p, element);
            }
            self.raw.len = len + 1;
        }
    }
    pub fn len(&self) -> usize {
        self.raw.len
    }
    pub fn push(&mut self, vm: &mut JsVirtualMachine, value: T) {
        self.reserve(vm, self.len() + 1);

        unsafe {
            let end = self.raw.data.as_mut_ptr().add(self.len());
            end.write(value);
            self.raw.len += 1;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            unsafe {
                self.raw.len -= 1;
                Some(self.raw.data.as_ptr().add(self.len()).read())
            }
        }
    }
}

impl<T: HeapObject> Index<usize> for GcVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.raw.data.as_ptr().add(index) }
    }
}

impl<T: HeapObject> IndexMut<usize> for GcVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.raw.data.as_mut_ptr().add(index) }
    }
}

impl<T: HeapObject> HeapObject for GcVec<T> {
    fn needs_destruction(&self) -> bool {
        false
    }

    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        self.raw.visit_children(tracer);
    }
}

impl<T: HeapObject> JsCell for GcVec<T> {}
