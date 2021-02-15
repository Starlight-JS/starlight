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
    gc::space::Space,
    heap::{
        cell::{Cell, Gc, Trace, Tracer},
        Allocator,
    },
    vm::VirtualMachine,
};

#[repr(C)]
pub struct GcArray<T: Cell> {
    /*len: u32,
    data: MaybeUninit<T>,*/
    data: Box<[T]>,
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

    pub fn new(space: &mut Space, size: usize, init: T) -> Gc<Self>
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

unsafe impl<T: Cell> Trace for GcArray<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for i in 0..self.len() {
            self[i].trace(tracer);
        }
    }
}

#[cfg(feature = "debug-snapshots")]
impl<T: Cell> serde::Serialize for GcArray<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("GcArray")?;
        x.serialize_field("len", self.len)?;
        x.serialize_field("data", unsafe {
            std::slice::from_raw_parts(self.data.as_ptr(), self.len)
        })?;
        x.end()
    }
}
impl<T: Cell> Cell for GcArray<T> {
    fn compute_size(&self) -> usize {
        /*(self.data.len() as usize * size_of::<T>()) +*/
        size_of::<Self>()
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

#[cfg(feature = "debug-snapshots")]
impl<T: Cell> serde::Serialize for RawVec<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("RawVec", 3)?;
        x.serialize_field("len", &self.len)?;
        x.serialize_field("cap", &self.cap)?;
        x.serialize_field("data", unsafe {
            std::slice::from_raw_parts(self.data.as_ptr(), self.len)
        })?;
        x.end()
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
/*

pub struct GcVec<T: Cell> {
    raw: Heap<RawVec<T>>,
}

impl<T: Cell> GcVec<T> {
    pub fn new(vm: &mut VirtualMachine, cap: usize) -> Self {
        Self {
            raw: RawVec::new(vm, cap),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn reserve(&mut self, vm: &mut VirtualMachine, n: usize) {
        if n > self.raw.cap as usize {
            let next = if n < 8 { 8 } else { clp2(n) };
            let mut ptr = RawVec::<T>::new(vm, next);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.raw.data.as_ptr(),
                    ptr.data.as_mut_ptr(),
                    self.raw.len as _,
                );
                std::ptr::write_bytes::<T>(
                    ptr.data.as_mut_ptr().add(self.raw.len as _),
                    0,
                    n - self.raw.len as usize,
                );
            }
            ptr.len = self.raw.len;

            self.raw = ptr;
        }
    }
    pub fn resize(&mut self, vm: &mut VirtualMachine, n: usize, val: T)
    where
        T: Clone,
    {
        let prev = self.raw.len;
        self.reserve(vm, n);
        self.raw.len = n as _;
        if prev < n as u32 {
            for i in prev..n as u32 {
                self[i as usize] = val.clone();
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
    pub fn shrink_to_fit(&mut self, vm: &mut VirtualMachine) {
        unsafe {
            let next = self.raw.len;
            let mut ptr = RawVec::<T>::new(vm, next as _);
            {
                std::ptr::copy_nonoverlapping(
                    self.raw.data.as_ptr(),
                    ptr.data.as_mut_ptr(),
                    self.raw.len as usize,
                );
            }
            ptr.len = self.raw.len;
            self.raw = ptr;
        }
    }

    pub fn insert(&mut self, vm: &mut VirtualMachine, index: usize, element: T) {
        #[cold]
        #[inline(never)]
        fn assert_failed(index: usize, len: usize) -> ! {
            panic!(
                "insertion index (is {}) should be <= len (is {})",
                index, len
            );
        }
        let len = self.raw.len;
        if index > self.raw.len as usize {
            assert_failed(index, len as _);
        }
        if len == self.raw.cap {
            self.reserve(vm, len as usize + 1);
        }
        unsafe {
            {
                let p = self.raw.data.as_mut_ptr().add(index as usize);
                std::ptr::copy(p, p.offset(1), len as usize - index as usize);
                std::ptr::write(p, element);
            }
            self.raw.len = len + 1;
        }
    }
    pub fn len(&self) -> usize {
        self.raw.len as _
    }
    pub fn push(&mut self, vm: &mut VirtualMachine, value: T) {
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

impl<T: Cell> Index<usize> for GcVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.raw.data.as_ptr().add(index) }
    }
}

impl<T: Cell> IndexMut<usize> for GcVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.raw.data.as_mut_ptr().add(index) }
    }
}

#[cfg(feature = "debug-snapshots")]
impl<T: Cell> serde::Serialize for GcVec<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("GcVec", 1)?;
        x.serialize_field("raw", self.raw);
        x.end()
    }
}

impl<T: Cell> Cell for GcVec<T> {}

impl<T: Cell> Trace for GcVec<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for i in 0..self.len() {
            self[i].trace(tracer);
        }
    }
}
impl<T: Cell> AsRef<[T]> for GcVec<T> {
    fn as_ref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.raw.data.as_ptr(), self.len()) }
    }
}

impl<T: Cell> AsMut<[T]> for GcVec<T> {
    fn as_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.raw.data.as_mut_ptr(), self.len()) }
    }
}

impl<T: Cell> Drop for RawVec<T> {
    fn drop(&mut self) {
        if !std::mem::needs_drop::<T>() {
            return;
        }

        for i in 0..self.len {
            unsafe {
                std::ptr::drop_in_place(self.data.as_mut_ptr().add(i as _));
            }
        }
    }
}*/

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
