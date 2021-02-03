use std::mem::size_of;

use crate::{
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};

use super::{
    js_cell::{allocate_cell, JsCell},
    ref_ptr::Ref,
    vm::JsVirtualMachine,
};

pub struct Context {
    vm: Ref<JsVirtualMachine>,
}
impl Context {
    pub fn new(vm: &mut JsVirtualMachine) -> Handle<Self> {
        let this = Self { vm: Ref::new(vm) };
        allocate_cell(vm, size_of::<Self>(), this)
    }
    #[allow(clippy::mut_from_ref)]
    pub fn get_vm(&self) -> &mut JsVirtualMachine {
        unsafe { &mut *self.vm.pointer }
    }
}

impl AsMut<JsVirtualMachine> for Handle<Context> {
    fn as_mut(&mut self) -> &mut JsVirtualMachine {
        {
            self.vm()
        }
    }
}
impl JsCell for Context {}
impl HeapObject for Context {
    fn visit_children(&mut self, _tracer: &mut dyn Tracer) {}
    fn needs_destruction(&self) -> bool {
        true
    }
}
/*
impl AsRefPtr<JsVirtualMachine> for Context {
    fn as_ref_ptr(&self) -> &mut JsVirtualMachine {
        self.vm
    }
}

impl AsRefPtr<JsVirtualMachine> for &Context {
    fn as_ref_ptr(&self) -> &mut JsVirtualMachine {
        self.vm
    }
}
impl AsRefPtr<JsVirtualMachine> for &mut Context {
    fn as_ref_ptr(&self) -> &mut JsVirtualMachine {
        self.vm
    }
}
*/
