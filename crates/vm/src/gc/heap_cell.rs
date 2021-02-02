use crate::{
    heap::{block::ImmixBlock, large_object_space::PreciseAllocation},
    runtime::ref_ptr::Ref,
};
use crate::{
    heap::{
        trace::Tracer,
        util::{address::Address, tagged_pointer::TaggedPointer},
    },
    runtime::{js_cell::JsCell, vm::JsVirtualMachine},
};
use mopa::{mopafy, Any};
use wtf_rs::TraitObject;
pub trait HeapObject: Any + JsCell {
    fn visit_children(&mut self, tracer: &mut dyn Tracer);
    fn compute_size(&self) -> usize {
        std::mem::size_of_val(self)
    }
    fn needs_destruction(&self) -> bool;
}

mopafy!(HeapObject);
/// The object is in eden. During GC, this means that the object has not been marked yet.
pub const GC_WHITE: u8 = 0x1;
// The object is either currently being scanned, or it has finished being scanned, or this
// is a full collection and it's actually a white object (you'd know because its mark bit
// would be clear).
pub const GC_BLACK: u8 = 0x0;
// This sorta means that the object is grey - i.e. it will be scanned. Or it could be white
// during a full collection if its mark bit is clear. That would happen if it had been black,
// got barriered, and we did a full collection.
pub const GC_GRAY: u8 = 0x2;

pub union HeapCellU {
    pub word: u64,
    pub tagged: TaggedPointer<u8>,
}

pub struct HeapCell {
    pub u: HeapCellU,
}
impl HeapCell {
    pub fn data(&self) -> Address {
        Address::from_ptr(self).offset(8)
    }
    pub fn vtable(&self) -> Address {
        unsafe { Address::from_ptr(self.u.tagged.untagged()) }
    }
    pub fn get_dyn(&self) -> &mut dyn HeapObject {
        unsafe {
            std::mem::transmute(TraitObject {
                data: self.data().to_mut_ptr(),
                vtable: self.vtable().to_mut_ptr(),
            })
        }
    }

    pub fn is_zapped(&self) -> bool {
        unsafe { self.u.word == 0 }
    }

    pub fn zap(&mut self) {
        self.u.word = 0;
    }

    pub fn tag(&self) -> u8 {
        unsafe { (self.u.word & 0x03) as _ }
    }
    pub fn set_vtable(&mut self, vtable: usize) {
        self.u.word = vtable as u64 | self.tag() as u64;
    }

    pub fn set_tag(&mut self, tag: u8) {
        self.u.word = self.vtable().to_usize() as u64 | tag as u64;
    }

    pub(crate) fn mark(&mut self, mark: bool) -> bool {
        unsafe {
            let prev = self.u.tagged.bit_is_set(1);
            self.u.tagged.set_bit_x(mark, 1);
            prev == mark
        }
    }

    pub(crate) fn get_mark(&self) -> bool {
        unsafe { self.u.tagged.bit_is_set(1) }
    }

    pub(crate) fn is_pinned(&self) -> bool {
        unsafe { self.u.tagged.bit_is_set(2) }
    }

    pub(crate) fn pin(&mut self) {
        unsafe {
            self.u.tagged.set_bit(2);
        }
    }

    pub(crate) fn unpin(&mut self) {
        unsafe {
            self.u.tagged.clear_bit(2);
        }
    }

    pub(crate) fn is_forwarded(&self) -> bool {
        unsafe { self.u.tagged.bit_is_set(0) }
    }

    pub(crate) fn set_forwarded(&mut self, addr: Address) {
        self.u.tagged = TaggedPointer::new(addr.to_mut_ptr());
        unsafe {
            self.u.tagged.set_bit(0);
        }
    }

    pub fn vm(&self) -> Ref<JsVirtualMachine> {
        if PreciseAllocation::is_precise(self as *const Self as *mut ()) {
            unsafe { (*PreciseAllocation::from_cell(self as *const Self as *mut _)).vm }
        } else {
            unsafe {
                let block = ImmixBlock::get_block_ptr(Address::from_ptr(self));
                (*block).vm
            }
        }
    }

    /// Use this function when you know that object is not allocated in large object space
    pub unsafe fn fast_vm(&self) -> Ref<JsVirtualMachine> {
        let block = ImmixBlock::get_block_ptr(Address::from_ptr(self));
        (*block).vm
    }
}
