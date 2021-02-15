use super::{
    attributes::*,
    error::{JsSyntaxError, JsTypeError},
    property_descriptor::DataDescriptor,
    slot::Slot,
    string::JsString,
};
use super::{object::JsObject, symbol::Symbol, value::JsValue};
use crate::{heap::cell::Gc, vm::VirtualMachine};
pub struct Env {
    pub record: Gc<JsObject>,
}

impl Env {
    pub fn is_mutable(&self, vm: &mut VirtualMachine, name: Symbol) -> bool {
        let prop = self.record.get_property(vm, name);

        prop.is_writable() && prop.raw != NONE
    }
    pub fn set_variable(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        strict: bool,
    ) -> Result<(Gc<JsObject>, Slot), JsValue> {
        if self.record.has_own_property(vm, name) {
            if !self.is_mutable(vm, name) && strict {
                let msg = JsString::new(vm, "Assignment to constant variable");
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            }
            let mut slot = Slot::new();
            self.record.put_slot(vm, name, val, &mut slot, strict)?;
            return Ok((self.record, slot));
        } else {
            let mut current = self.record.prototype();
            while let Some(mut cur) = current {
                if cur.has_own_property(vm, name) {
                    let prop = cur.get_property(vm, name);
                    if !(prop.is_writable() && prop.raw != NONE) && strict {
                        let msg = JsString::new(vm, "Assignment to constant variable");
                        return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
                    }
                    let mut slot = Slot::new();
                    cur.put_slot(vm, name, val, &mut slot, strict)?;
                    return Ok((cur, slot));
                }
                current = cur.prototype();
            }

            if strict {
                let desc = vm.description(name);
                let msg = JsString::new(vm, format!("Variable '{}' does not exist", desc));
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            } else {
                let mut slot = Slot::new();
                vm.global_object()
                    .put_slot(vm, name, val, &mut slot, false)?;
                return Ok((vm.global_object(), slot));
            }
        }
    }
    pub fn get_variable(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
    ) -> Result<JsValue, JsValue> {
        self.record.get(vm, name)
    }
    pub fn has_own_variable(&mut self, vm: &mut VirtualMachine, name: Symbol) -> bool {
        self.record.has_own_property(vm, name)
    }
    pub fn declare_variable(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        mutable: bool,
    ) -> Result<(), JsValue> {
        let desc = DataDescriptor::new(JsValue::undefined(), E | C | if mutable { W } else { 0 });

        if self.has_own_variable(vm, name) {
            let desc = vm.description(name);
            let msg = JsString::new(
                vm,
                format!("Identifier '{}' already exists in this scope", desc),
            );
            return Err(JsValue::new(JsSyntaxError::new(vm, msg, None)));
        }

        let _ = self.record.define_own_property(vm, name, &*desc, false);
        Ok(())
    }
}
