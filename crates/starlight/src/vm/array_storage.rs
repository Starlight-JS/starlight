use std::mem::size_of;

use super::value::JsValue;
use crate::heap::{
    cell::{GcCell, GcPointer, Trace},
    Heap, SlotVisitor,
};
/// A GC-managed resizable vector of values. It is used for storage of property
/// values in objects and also indexed property values in arrays. It supports
/// resizing on both ends which is necessary for the simplest implementation of
/// JavaScript arrays (using a base offset and length).
/// Note: ArrayStorage does not support direct serialization as a heap object.
/// However a convenience method is provided to serialize an ArrayStorage, as
/// part of the record of its owning object. In this case the ArrayStorage must
/// not contain any native pointers.
#[repr(C)]
pub struct ArrayStorage {
    size: u32,
    capacity: u32,
    data: [JsValue; 0],
}

impl GcPointer<ArrayStorage> {
    pub fn resize_within_capacity(&mut self, _rt: &mut Heap, new_size: u32) {
        assert!(
            new_size <= self.capacity(),
            "new_size must be <= capacity in resize_Within_capacity"
        );

        let sz = self.size();
        unsafe {
            if new_size > sz {
                JsValue::fill(
                    self.data_mut().add(sz as _),
                    self.data_mut().add(new_size as _),
                    JsValue::encode_empty_value(),
                );
            }
        }
        self.size = new_size;
    }

    pub fn ensure_capacity(&mut self, rt: &mut Heap, capacity: u32) {
        assert!(
            capacity <= ArrayStorage::max_elements() as u32,
            "capacity overflows 32-bit storage"
        );

        if capacity <= self.capacity() {
            return;
        }

        unsafe { self.reallocate_to_larger(rt, capacity, 0, 0, self.size()) }
    }
    pub fn resize(&mut self, rt: &mut Heap, new_size: u32) {
        self.shift(rt, 0, 0, new_size)
    }

    #[cold]
    pub fn push_back_slowpath(&mut self, rt: &mut Heap, value: JsValue) {
        let size = self.size();

        self.resize(rt, self.size() + 1);
        *self.at_mut(size) = value;
    }

    pub fn push_back(&mut self, rt: &mut Heap, value: JsValue) {
        let currsz = self.size();
        if currsz < self.capacity() {
            unsafe {
                self.data_mut().add(currsz as _).write(value);
                self.size = currsz + 1;
            }
            return;
        }
        self.push_back_slowpath(rt, value)
    }

    pub fn pop_back(&mut self, _rt: &mut Heap) -> JsValue {
        let sz = self.size();
        assert!(sz > 0, "empty ArrayStorage");

        unsafe {
            let val = self.data().add(sz as usize - 1).read();
            self.size = sz - 1;
            val
        }
    }
    pub fn at(&self, index: u32) -> &JsValue {
        assert!(index < self.size(), "index out of range");
        unsafe { &*self.data().add(index as _) }
    }
    pub fn at_mut(&mut self, index: u32) -> &mut JsValue {
        assert!(index < self.size(), "index out of range");
        unsafe { &mut *self.data_mut().add(index as _) }
    }
    pub fn shift(&mut self, rt: &mut Heap, from_first: u32, to_first: u32, to_last: u32) {
        assert!(to_first <= to_last, "First must be before last");
        assert!(from_first <= self.size, "from_first must be before size");
        unsafe {
            if to_last <= self.capacity() {
                let copy_size = std::cmp::min(self.size() - from_first, to_last - to_first);
                if from_first > to_first {
                    JsValue::copy(
                        self.data_mut().add(from_first as usize),
                        self.data_mut()
                            .add(from_first as usize + copy_size as usize),
                        self.data_mut().add(to_first as usize),
                    );
                } else if from_first < to_first {
                    JsValue::copy_backward(
                        self.data_mut().add(from_first as usize),
                        self.data_mut()
                            .add(from_first as usize + copy_size as usize),
                        self.data_mut().add(to_first as _),
                    );
                }
                JsValue::fill(
                    self.data_mut().add(to_first as usize + copy_size as usize),
                    self.data_mut().add(to_last as usize),
                    JsValue::encode_empty_value(),
                );
                self.size = to_last;
                return;
            }

            let mut capacity = self.capacity();
            if capacity < ArrayStorage::max_elements() as u32 / 2 {
                capacity = std::cmp::max(capacity * 2, to_last);
            } else {
                capacity = ArrayStorage::max_elements() as u32;
            }
            self.reallocate_to_larger(rt, capacity, from_first, to_first, to_last)
        }
    }

    pub unsafe fn reallocate_to_larger(
        &mut self,
        rt: &mut Heap,
        capacity: u32,
        from_first: u32,
        to_first: u32,
        to_last: u32,
    ) {
        assert!(capacity > self.capacity());

        let mut arr_res = ArrayStorage::new(rt, capacity);
        let copy_size = std::cmp::min(self.size() - from_first, to_last - to_first);

        {
            let from = self.data_mut().add(from_first as _);
            let to = arr_res.data_mut().add(to_first as _);
            JsValue::uninit_copy(from, from.add(copy_size as _), to);
        }

        JsValue::fill(
            arr_res.data_mut(),
            arr_res.data_mut().add(to_first as _),
            JsValue::encode_empty_value(),
        );

        if to_first + copy_size < to_last {
            JsValue::fill(
                arr_res
                    .data_mut()
                    .add(to_first as usize + copy_size as usize),
                arr_res.data_mut().add(to_last as usize),
                JsValue::encode_empty_value(),
            );
        }

        arr_res.size = to_last;
        *self = arr_res;
    }
}

impl ArrayStorage {
    pub fn max_elements() -> usize {
        (u32::MAX as usize - 8) / size_of::<JsValue>()
    }
    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    pub fn with_size(rt: &mut Heap, size: u32, capacity: u32) -> GcPointer<Self> {
        let mut this = Self::new(rt, capacity);
        this.resize_within_capacity(rt, size);
        this
    }
    pub fn new(rt: &mut Heap, capacity: u32) -> GcPointer<Self> {
        let cell = rt.allocate(Self {
            capacity,
            size: 0,
            data: [],
        });

        cell
    }
    pub fn data(&self) -> *const JsValue {
        self.data.as_ptr()
    }
    pub fn as_slice(&self) -> &[JsValue] {
        unsafe { std::slice::from_raw_parts(self.data(), self.size as _) }
    }

    pub fn data_mut(&mut self) -> *mut JsValue {
        self.data.as_mut_ptr()
    }
    pub fn as_slice_mut(&mut self) -> &mut [JsValue] {
        unsafe { std::slice::from_raw_parts_mut(self.data_mut(), self.size as _) }
    }
}

unsafe impl Trace for ArrayStorage {
    fn trace(&self, visitor: &mut SlotVisitor) {
        self.as_slice().iter().for_each(|value| {
            value.trace(visitor);
        });
    }
}

impl GcCell for ArrayStorage {
    fn compute_size(&self) -> usize {
        (self.capacity as usize * size_of::<JsValue>()) + size_of::<Self>()
    }
}
