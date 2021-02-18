use std::mem::ManuallyDrop;

use crate::{heap::cell::Gc, vm::VirtualMachine};

use super::object::JsObject;
use super::{method_table::*, object::ObjectTag};

pub struct JsNumber {
    value: f64,
}

impl JsNumber {
    define_jsclass_with_symbol!(JsObject, Number, Number);

    pub fn new(vm: &mut VirtualMachine, number: f64) -> Gc<JsObject> {
        unsafe {
            let mut jsobject = JsObject::new(
                vm,
                vm.global_data().number_structure.unwrap(),
                Self::get_class(),
                ObjectTag::Number,
            );

            *jsobject.data::<Self>() = ManuallyDrop::new(Self { value: number });
            jsobject
        }
    }
    pub fn value(&self) -> f64 {
        self.value
    }
}
