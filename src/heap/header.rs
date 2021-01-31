use super::util::{address::Address, tagged_pointer::TaggedPointer};
use crate::runtime::type_info::TypeInfo;

/// # GC object header.
/// This structure encodes important data for garbage collection:
/// - mark, pin and forwarding bit
/// - vtable for tracing object
/// - forwarding pointer when forwarding bit is set.
///
/// # Usage
/// `Header` must be first field in the structure hence all GCed objects
/// should use `#[repr(C)]`.  When initializing on-stack value for allocation
/// `Header::empty` method should be used.
///
pub struct Header {
    rtti: TaggedPointer<TypeInfo>,
}

impl Header {
    /// Allocate new empty header.
    pub const fn empty() -> Self {
        Self {
            rtti: TaggedPointer::null(),
        }
    }
    pub(crate) fn type_info_ptr(&self) -> Address {
        Address::from_ptr(self.rtti.untagged())
    }
    /// Returns type info associated with current object.
    pub fn type_info(&self) -> &'static TypeInfo {
        unsafe { &*self.rtti.untagged().cast::<TypeInfo>() }
    }
    pub fn object_start(&self) -> Address {
        Address::from_ptr(self)
    }
    pub fn size(&self) -> usize {
        (self.type_info().heap_size)(self.object_start())
    }

    pub(crate) fn new(type_info: &'static TypeInfo) -> Self {
        Self {
            rtti: TaggedPointer::new(type_info as *const TypeInfo as *mut TypeInfo),
        }
    }

    pub(crate) fn mark(&mut self, mark: bool) -> bool {
        let prev = self.rtti.bit_is_set(1);
        self.rtti.set_bit_x(mark, 1);
        prev == mark
    }

    pub(crate) fn get_mark(&self) -> bool {
        self.rtti.bit_is_set(1)
    }

    pub(crate) fn is_pinned(&self) -> bool {
        self.rtti.bit_is_set(2)
    }

    pub(crate) fn pin(&mut self) {
        self.rtti.set_bit(2);
    }

    pub(crate) fn unpin(&mut self) {
        self.rtti.clear_bit(2);
    }

    pub(crate) fn is_forwarded(&self) -> bool {
        self.rtti.bit_is_set(0)
    }

    pub(crate) fn set_forwarded(&mut self, addr: Address) {
        self.rtti = TaggedPointer::new(addr.to_mut_ptr());
        self.rtti.set_bit(0);
    }
}
