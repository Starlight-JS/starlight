use std::ptr::NonNull;

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::header::Header,
};

use super::{ref_ptr::AsRefPtr, ref_ptr::Ref, type_info::*, vm::JsVirtualMachine};

pub fn allocate_cell<T: HeapObject>(
    vm: impl AsRefPtr<JsVirtualMachine>,
    size: usize,
    value: T,
) -> Handle<T> {
    let memory = unsafe { vm.as_ref_ptr().heap.allocate(value, size) };

    Handle::<T> {
        cell: unsafe { NonNull::new_unchecked(memory.to_mut_ptr()) },
        marker: Default::default(),
    }
}

pub trait JsCell {
    fn get_map(&self, vm: Ref<JsVirtualMachine>) -> () {
        todo!()
    }
}
