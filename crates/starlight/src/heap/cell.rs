use std::{
    marker::PhantomData,
    mem::size_of,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use mopa::mopafy;

use super::{precise_allocation::PreciseAllocation, SlotVisitor};

pub unsafe trait Trace {
    fn trace(&self, visitor: &mut SlotVisitor) {
        let _ = visitor;
    }
}

pub trait GcCell: mopa::Any + Trace {
    fn compute_size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

mopafy!(GcCell);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct GcPointerBase {
    vtable: u64,
}

impl GcPointerBase {
    pub fn new(vtable: usize) -> Self {
        Self {
            vtable: vtable as _,
        }
    }
    pub fn data<T>(&self) -> *mut T {
        unsafe {
            (self as *const Self as *mut u8)
                .add(size_of::<Self>())
                .cast()
        }
    }
    pub fn raw(&self) -> u64 {
        self.vtable
    }
    pub fn is_live(&self) -> bool {
        ((self.vtable >> 1) & 1) == 1
    }

    pub fn is_marked(&self) -> bool {
        ((self.vtable >> 0) & 1) == 1
    }

    pub fn mark(&mut self) {
        self.vtable |= 1 << 0;
    }

    pub fn unmark(&mut self) {
        self.vtable &= !(1 << 0);
    }

    pub fn live(&mut self) {
        self.vtable |= 1 << 1;
    }
    pub fn dead(&mut self) {
        self.vtable &= !(1 << 1);
    }

    pub fn get_dyn(&self) -> &mut dyn GcCell {
        unsafe {
            std::mem::transmute(mopa::TraitObject {
                vtable: (self.vtable & (!0x03)) as *mut (),
                data: self.data::<u8>() as _,
            })
        }
    }
    pub fn is_precise_allocation(&self) -> bool {
        PreciseAllocation::is_precise(self as *const Self as *mut ())
    }

    pub fn precise_allocation(&self) -> *mut PreciseAllocation {
        PreciseAllocation::from_cell(self as *const Self as *mut _)
    }
    pub fn vtable(&self) -> usize {
        (self.vtable & (!0x07)) as usize
    }
}

pub struct GcPointer<T: ?Sized> {
    pub(super) base: NonNull<GcPointerBase>,
    pub(super) marker: PhantomData<T>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakState {
    Free = 0,
    Unmarked,
    Mark,
}
pub struct WeakSlot {
    pub(super) state: WeakState,
    pub(super) value: *mut GcPointerBase,
}

pub struct WeakRef<T: GcCell> {
    pub(super) inner: NonNull<WeakSlot>,
    pub(super) marker: PhantomData<T>,
}

impl<T: GcCell> WeakRef<T> {
    pub fn upgrade(&self) -> Option<GcPointer<T>> {
        unsafe {
            let inner = &*self.inner.as_ptr();
            if inner.value.is_null() {
                return None;
            }

            Some(GcPointer {
                base: NonNull::new_unchecked(inner.value),
                marker: PhantomData::<T>,
            })
        }
    }
}

macro_rules! impl_prim {
    ($($t: ty)*) => {
        $(
            unsafe impl Trace for $t {}
            impl GcCell for $t {}
        )*
    };
}

impl_prim!(String bool f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 u128 i128);
unsafe impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, visitor: &mut SlotVisitor) {
        for val in self.iter() {
            val.trace(visitor);
        }
    }
}

unsafe impl<T: GcCell> Trace for WeakRef<T> {
    fn trace(&self, visitor: &mut SlotVisitor) {
        visitor.visit_weak(self);
    }
}

unsafe impl<T: GcCell> Trace for GcPointer<T> {
    fn trace(&self, visitor: &mut SlotVisitor) {
        visitor.visit(*self);
    }
}

impl<T: GcCell> Copy for GcPointer<T> {}
impl<T: GcCell> Clone for GcPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GcCell> Deref for GcPointer<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(&*self.base.as_ptr()).data::<T>() }
    }
}
impl<T: GcCell> DerefMut for GcPointer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(&*self.base.as_ptr()).data::<T>() }
    }
}

impl<T: GcCell> std::fmt::Pointer for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:p}", self.base)
    }
}
