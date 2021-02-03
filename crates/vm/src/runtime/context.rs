use super::{
    ref_ptr::{AsRefPtr, Ref},
    vm::JsVirtualMachine,
};

pub struct Context {
    pub vm: Ref<JsVirtualMachine>,
}
impl Context {
    pub fn new(vm: &mut JsVirtualMachine) -> Ref<Self> {
        Ref::new(Box::into_raw(Box::new(Self { vm: Ref::new(vm) })))
    }
}
impl AsRefPtr<JsVirtualMachine> for Context {
    fn as_ref_ptr(&self) -> Ref<JsVirtualMachine> {
        self.vm
    }
}

impl AsRefPtr<JsVirtualMachine> for &Context {
    fn as_ref_ptr(&self) -> Ref<JsVirtualMachine> {
        self.vm
    }
}
impl AsRefPtr<JsVirtualMachine> for &mut Context {
    fn as_ref_ptr(&self) -> Ref<JsVirtualMachine> {
        self.vm
    }
}
