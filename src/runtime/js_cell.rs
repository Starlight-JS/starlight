use super::{ref_ptr::AsRefPtr, ref_ptr::Ref, type_info::*, vm::JSVirtualMachine};

pub fn allocate_cell<T: Type>(
    vm: impl AsRefPtr<JSVirtualMachine>,
    size: usize,
    ty_info: &'static TypeInfo,
    value: T,
) -> Ref<T> {
    let memory = unsafe { vm.as_ref_ptr().heap.allocate(size, ty_info) };
    unsafe {
        memory.to_mut_ptr::<T>().write(value);
    }
    Ref::new(memory.to_mut_ptr())
}
