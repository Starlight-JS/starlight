use super::{header::Header, util::address};
use address::Address;

pub struct Slot {
    pub(crate) addr: Address,
}

impl Slot {
    pub fn new<T>(hdr_ptr: &mut T) -> Self {
        Self {
            addr: Address::from_ptr(hdr_ptr as *mut T as *mut *mut Header),
        }
    }

    pub(crate) fn set(&self, to: Address) {
        unsafe {
            self.addr
                .to_mut_ptr::<*mut Header>()
                .write(to.to_mut_ptr::<Header>());
        }
    }

    pub(crate) fn value(&self) -> *mut Header {
        unsafe { *self.addr.to_mut_ptr::<*mut Header>() }
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
