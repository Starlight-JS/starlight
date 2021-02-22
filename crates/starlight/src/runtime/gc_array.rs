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
    mem::{size_of, MaybeUninit},
    ops::{Index, IndexMut},
    usize,
};

use crate::{
    gc::cell::{Cell, Gc, Trace, Tracer},
    heap::Allocator,
    vm::VirtualMachine,
};
/*
#[repr(C)]
pub struct GcArray<T: Cell> {
    /*len: u32,
    data: MaybeUninit<T>,*/
    pub(crate) data: Box<[T]>,
}
impl<T: Cell> GcArray<T> {
    pub fn begin(&self) -> *mut T {
        self.data.as_ptr() as *mut _
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn end(&self) -> *mut T {
        unsafe { self.begin().add(self.data.len() as _) }
    }
    /*pub fn new(vm: &mut JsVirtualMachine, len: usize) -> Handle<Self>
    where
        T: Default,
    {
        //  println!("alloc");
        let val = Self { len, data: [] };
        let mut cell = allocate_cell(vm, len * size_of::<T>() + size_of::<GcArray<T>>(), val);
        for i in 0..cell.len() {
            cell[i] = T::default();
        }
        cell
    }*/

    pub fn new(space: &mut Heap, size: usize, init: T) -> Gc<Self>
    where
        T: Clone,
    {
        let val = Self {
            data: vec![init; size].into_boxed_slice(),
        };
        space.alloc(val)
    }
    pub fn len(&self) -> usize {
        self.data.len() as _
    }
}
*/
#[repr(C)]
pub struct GcArray<T: Cell> {
    length: usize,
    data: [T; 0],
}

impl<T: Cell> GcArray<T> {
    pub fn new(vm: &mut VirtualMachine, len: usize, init: T) -> Gc<Self>
    where
        T: Clone,
    {
        //  println!("alloc");
        let val = Self {
            length: len,
            data: [],
        };
        let mut cell = vm.space().alloc(val);
        for i in 0..cell.len() {
            unsafe {
                cell.begin().add(i as usize).write(init.clone());
            }
        }
        cell
    }
    pub fn begin(&self) -> *mut T {
        self.data.as_ptr() as *mut _
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn end(&self) -> *mut T {
        unsafe { self.begin().add(self.data.len() as _) }
    }

    pub fn len(&self) -> usize {
        self.length
    }
}
unsafe impl<T: Cell> Trace for GcArray<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for i in 0..self.len() {
            self[i].trace(tracer);
        }
    }
}

impl<T: Cell> Cell for GcArray<T> {
    fn compute_size(&self) -> usize {
        (self.length as usize * size_of::<T>()) + size_of::<Self>()
        //size_of::<Self>()
    }
}
impl<T: Cell> Index<usize> for GcArray<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.data.as_ptr().add(index) }
    }
}
impl<T: Cell> IndexMut<usize> for GcArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.data.as_mut_ptr().add(index) }
    }
}

impl<T: Cell> AsRef<[T]> for GcArray<T> {
    fn as_ref(&self) -> &[T] {
        &self.data
        //unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len as usize) }
    }
}
impl<T: Cell> AsMut<[T]> for GcArray<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.data //unsafe { std::slice::from_raw_parts_mut(self.data.as_ptr() as *mut T, self.len as usize) }
    }
}

#[repr(C)]
struct RawVec<T: Cell> {
    cap: u32,
    len: u32,
    data: MaybeUninit<T>,
}

impl<T: Cell> RawVec<T> {
    pub fn new(vm: &mut VirtualMachine, len: usize) -> Gc<Self> {
        let val = Self {
            len: 0,
            cap: len as _,
            data: MaybeUninit::uninit(),
        };
        vm.space().allocate(val)
    }
}

impl<T: Cell> Cell for RawVec<T> {
    fn compute_size(&self) -> usize {
        (self.cap as usize * size_of::<T>()) + size_of::<Self>()
    }
}

unsafe impl<T: Cell> Trace for RawVec<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for i in 0..self.len {
            unsafe {
                (*self.data.as_ptr().add(i as _)).trace(tracer);
            }
        }
    }
}

impl<T: Cell> Index<usize> for RawVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.data.as_ptr().add(index) }
    }
}
impl<T: Cell> IndexMut<usize> for RawVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.data.as_mut_ptr().add(index) }
    }
}

pub struct GcVec<T: Cell> {
    data: Gc<Vec<T>>,
}

impl<T: Cell> GcVec<T> {
    pub fn new(vm: &mut VirtualMachine, cap: usize) -> Self {
        Self {
            data: vm.allocate(Vec::with_capacity(cap)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }

    pub fn reserve(&mut self, _: &mut VirtualMachine, n: usize) {
        self.data.reserve(n);
    }

    pub fn resize(&mut self, _vm: &mut VirtualMachine, n: usize, data: T)
    where
        T: Clone,
    {
        self.data.resize(n, data)
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn shrink_to_fit(&mut self, _vm: &mut VirtualMachine) {
        self.data.shrink_to_fit();
    }

    pub fn insert(&mut self, _vm: &mut VirtualMachine, at: usize, val: T) {
        self.data.insert(at, val);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn push(&mut self, _vm: &mut VirtualMachine, val: T) {
        self.data.push(val);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.data.pop()
    }
}

impl<T: Cell> Index<usize> for GcVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T: Cell> IndexMut<usize> for GcVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl<T: Cell> Cell for GcVec<T> {}
unsafe impl<T: Trace + Cell> Trace for GcVec<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.data.trace(tracer);
    }
}
