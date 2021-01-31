use crate::heap::header::Header;

use super::{ref_ptr::AsRefPtr, ref_ptr::Ref, type_info::*, vm::JSVirtualMachine};

pub fn allocate_cell<T: Type>(
    vm: impl AsRefPtr<JSVirtualMachine>,
    size: usize,
    ty_info: &'static TypeInfo,
    value: T,
) -> Ref<T> {
    let memory = unsafe { vm.as_ref_ptr().heap.allocate(value, size, ty_info) };

    Ref::new(memory.to_mut_ptr())
}

#[repr(C)]
pub struct JSCell {
    pub(crate) header: Header,
}

impl JSCell {
    pub fn vm(&self) -> Ref<JSVirtualMachine> {
        self.header.vm()
    }
}
