use super::{
    env::Env, method_table::*, object::*, property_descriptor::*, slot::*, symbol::*, value::*,
};
use super::{error::JsTypeError, string::JsString};
use crate::{heap::cell::*, vm::*};
use std::mem::ManuallyDrop;

use super::{
    object::{JsObject, ObjectTag},
    symbol::Symbol,
};

pub struct JsArguments {
    // TODO: Better alternative?
    pub mapping: Box<[Symbol]>,
    pub env: Gc<JsObject>,
}
#[allow(non_snake_case)]
impl JsArguments {
    define_jsclass!(JsArguments, Arguments);
    pub fn GetPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        mut obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        _slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        match obj.define_own_indexed_property_internal(vm, index, desc, throwable) {
            Ok(false) | Err(_) => {
                if throwable {
                    let msg = JsString::new(vm, "[[DefineOwnProperty]] failed");
                    return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
                }
                return Ok(false);
            }
            _ => {
                let arg = obj.as_arguments_mut();
                if arg.mapping.len() > index as usize {
                    let mapped = arg.mapping[index as usize];
                    if mapped != DUMMY_SYMBOL {
                        if desc.is_accessor() {
                            arg.mapping[index as usize] = DUMMY_SYMBOL;
                        } else {
                            if desc.is_data() {
                                let data = DataDescriptor { parent: *desc };
                                if !data.is_value_absent() {
                                    Env { record: arg.env }.set_variable(
                                        vm,
                                        mapped,
                                        desc.value(),
                                        throwable,
                                    )?;
                                }

                                if !data.is_writable_absent() && !data.is_writable() {
                                    arg.mapping[index as usize] = DUMMY_SYMBOL;
                                }
                            }
                        }
                    }
                }

                Ok(true)
            }
        }
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if !JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot) {
            return false;
        }
        let arg = obj.as_arguments();

        if arg.mapping.len() > index as usize {
            let mapped = arg.mapping[index as usize];
            if mapped != DUMMY_SYMBOL {
                let val = arg
                    .env
                    .get(vm, mapped)
                    .unwrap_or_else(|_| JsValue::undefined());
                let attrs = slot.attributes();
                slot.set(val, attrs);
                return true;
            }
        }
        true
    }
    pub fn PutIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let v = JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot);
        if name == Symbol::caller() {
            match v {
                Ok(x) if x.is_callable() => {
                    if x.as_cell()
                        .downcast::<JsObject>()
                        .unwrap()
                        .as_function()
                        .is_strict()
                    {
                        let msg =
                            JsString::new(vm, "access to strict function 'caller' not allowed");
                        return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
                    }
                }
                _ => (),
            }
        }
        v
    }

    pub fn GetIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        //!();
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn new(vm: &mut VirtualMachine, env: Gc<JsObject>, params: &[Symbol]) -> Gc<JsObject> {
        let struct_ = vm.global_data().normal_arguments_structure.unwrap();
        let mut obj = JsObject::new(
            vm,
            struct_,
            JsArguments::get_class(),
            ObjectTag::NormalArguments,
        )
        .root();
        //let s = Structure::new_unique_indexed(vm, None, true);

        let args = JsArguments {
            mapping: vec![].into_boxed_slice(),
            env,
        };
        *obj.data::<JsArguments>() = ManuallyDrop::new(args);
        use super::attributes::*;
        let mut mapping = Vec::with_capacity(params.len());
        for (i, param) in params.iter().enumerate() {
            let mut slot = Slot::new();
            let _ = obj.define_own_indexed_property_slot(
                vm,
                i as _,
                &*DataDescriptor::new(
                    JsValue::new(2),
                    create_data(AttrExternal::new(Some(W | C | E))).raw(),
                ),
                &mut slot,
                false,
            );

            mapping.push(*param);

            //let _ = obj.put(vm, Symbol::Indexed(i as _), JsValue::undefined(), false);
        }
        obj.as_arguments_mut().mapping = mapping.into_boxed_slice();
        *obj
    }
}

unsafe impl Trace for JsArguments {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.env.trace(tracer);
    }
}
