use super::method_table::MethodTable;
use super::{
    attributes::*, object::*, property_descriptor::*, slot::*, structure::*, symbol::*, value::*,
};
use crate::heap::cell::*;
use crate::vm::*;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use wtf_rs::pure_nan;
use wtf_rs::segmented_vec::SegmentedVec;
pub struct JsGlobal {
    sym_map: HashMap<Symbol, u32>,
    variables: SegmentedVec<StoredSlot>,
    vm: VirtualMachineRef,
}

#[allow(non_snake_case)]
impl JsGlobal {
    pub fn new(vm: &mut VirtualMachine) -> Gc<JsObject> {
        let shape = Structure::new_unique_with_proto(vm, None, false);
        let mut js_object = JsObject::new(vm, shape, Self::get_class(), ObjectTag::Global);
        unsafe {
            *js_object.data::<JsGlobal>() = ManuallyDrop::new(Self {
                sym_map: Default::default(),
                variables: SegmentedVec::with_chunk_size(8),
                vm: VirtualMachineRef(vm),
            });
        }
        js_object
    }
    define_jsclass!(JsGlobal, global);
    pub fn lookup_constant(&self, name: Symbol) -> Option<JsValue> {
        let _vm = self.vm;
        if name == Symbol::Infinity() {
            Some(JsValue::new(std::f64::INFINITY))
        } else if name == Symbol::NaN() {
            Some(JsValue::new(pure_nan::pure_nan()))
        } else if name == Symbol::undefined() {
            Some(JsValue::undefined())
        } else {
            None
        }
    }

    pub fn lookup_variable(&self, name: Symbol) -> Option<u32> {
        self.sym_map.get(&name).copied()
    }
    pub fn push_variable(&mut self, name: Symbol, init: JsValue, attributes: AttrSafe) {
        self.sym_map.insert(name, self.variables.len() as _);
        self.variables.push(StoredSlot::new_raw(init, attributes));
    }

    pub fn point_at(&self, x: u32) -> &StoredSlot {
        &self.variables[x as usize]
    }

    pub fn point_at_mut(&mut self, x: u32) -> &mut StoredSlot {
        &mut self.variables[x as usize]
    }
    pub fn variables(&self) -> &SegmentedVec<StoredSlot> {
        &self.variables
    }

    pub fn variables_mut(&mut self) -> &mut SegmentedVec<StoredSlot> {
        &mut self.variables
    }

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
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
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
        for it in obj.as_global().sym_map.iter() {
            collector(*it.0, *it.1);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let entry = obj.as_global().lookup_variable(name);
        if entry.is_some() {
            // all variables are configurable: false
            return Ok(false);
        }
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
        JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
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
        let global = obj.as_global();

        let entry = global.lookup_variable(name);

        if let Some(entry) = entry {
            let stored = &global.variables[entry as usize];

            slot.set_1(stored.value(), stored.attributes(), Some(obj.as_dyn()));

            return true;
        }

        let res = JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot);
        if !res {
            slot.make_uncacheable();
        }
        res
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
        let global = obj.as_global_mut();
        let entry = global.lookup_variable(name);
        if let Some(entry) = entry {
            let mut stored = global.variables[entry as usize];
            let mut returned = false;
            if stored.is_defined_property_accepted(vm, desc, throwable, &mut returned)? {
                stored.merge(vm, desc);
                global.variables[entry as usize] = stored;
            }
            return Ok(returned);
        }
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
}

unsafe impl Trace for JsGlobal {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for var in self.variables.iter() {
            var.trace(tracer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_obj() {
        let mut vm = VirtualMachine::new(Options::default());

        let mut global = JsGlobal::new(&mut vm).root(vm.space());
        let attr = create_data(AttrExternal::new(Some(W | E)));
        assert!(attr.is_data() && !attr.is_accessor());
        global
            .as_global_mut()
            .push_variable(vm.intern("x"), JsValue::new(3.0), attr);
        let x = vm.intern("x");
        let var = global.get(&mut vm, x);

        match var {
            Err(_) => panic!("variable not found"),
            Ok(x) => {
                assert!(x.is_number());
                assert_eq!(x.number(), 3.0);
            }
        }
        drop(global);

        VirtualMachineRef::dispose(vm);
    }
}
