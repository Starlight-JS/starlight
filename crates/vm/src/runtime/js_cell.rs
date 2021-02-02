use std::ptr::NonNull;

use crate::gc::{handle::Handle, heap_cell::HeapObject};

use super::{ref_ptr::AsRefPtr, ref_ptr::Ref, structure::Structure, vm::JsVirtualMachine};

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

#[allow(unused_variables)]
pub trait JsCell {
    fn get_structure(&self, vm: Ref<JsVirtualMachine>) -> Handle<Structure> {
        todo!()
    }

    fn set_structure(&mut self, vm: Ref<JsVirtualMachine>, s: Handle<Structure>) {
        todo!()
    }
}
