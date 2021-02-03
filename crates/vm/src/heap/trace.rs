use crate::gc::{
    handle::Handle,
    heap_cell::{HeapCell, HeapObject},
};

use super::util::address;
use address::Address;

pub struct Slot {
    pub(crate) addr: Address,
}

impl<T: HeapObject + ?Sized> From<&mut Handle<T>> for Slot {
    fn from(val: &mut Handle<T>) -> Slot {
        Slot::new(val)
    }
}

impl Slot {
    pub fn new<T>(hdr_ptr: &mut T) -> Self {
        Self {
            addr: Address::from_ptr(hdr_ptr as *mut T as *mut *mut HeapCell),
        }
    }

    pub(crate) fn set(&self, to: Address) {
        unsafe {
            self.addr
                .to_mut_ptr::<*mut HeapCell>()
                .write(to.to_mut_ptr::<HeapCell>());
        }
    }

    pub(crate) fn value(&self) -> *mut HeapCell {
        unsafe { *self.addr.to_mut_ptr::<*mut HeapCell>() }
    }
}

pub trait Tracer {
    fn trace(&mut self, slot: Slot);
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TracerPtr {
    pub(crate) tracer: [usize; 2],
}

impl TracerPtr {
    pub fn new<'a>(x: &'a mut dyn Tracer) -> Self {
        Self {
            tracer: unsafe { core::mem::transmute(x) },
        }
    }

    pub fn trace(self, slot: Slot) {
        unsafe {
            (*core::mem::transmute::<[usize; 2], *mut dyn Tracer>(self.tracer))
                .trace(core::mem::transmute(slot));
        }
    }
}

pub struct SimpleVisitor<'a> {
    closure: &'a mut dyn FnMut(Slot),
}

impl<'a> SimpleVisitor<'a> {
    pub fn new(closure: &'a mut dyn FnMut(Slot)) -> Self {
        Self { closure }
    }
}

impl<'a> Tracer for SimpleVisitor<'a> {
    fn trace(&mut self, slot: Slot) {
        (self.closure)(slot);
    }
}
