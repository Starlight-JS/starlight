use std::mem::ManuallyDrop;

use super::{
    array_storage::ArrayStorage,
    error::JsTypeError,
    method_table::*,
    object::{EnumerationMode, Env, JsHint, JsObject, ObjectTag},
    property_descriptor::*,
    slot::*,
    string::JsString,
    symbol_table::Internable,
    symbol_table::{Symbol, DUMMY_SYMBOL},
    value::*,
    Runtime,
};
use crate::heap::cell::{GcCell, GcPointer, Trace, Tracer};

pub struct Arguments {
    pub this: JsValue,
    pub values: GcPointer<ArrayStorage>,
    pub ctor_call: bool,
}

impl Arguments {
    pub fn from_array_storage(
        _rt: &mut Runtime,
        this: JsValue,
        values: GcPointer<ArrayStorage>,
    ) -> Self {
        Self {
            this,
            values,
            ctor_call: false,
        }
    }
    pub fn size(&self) -> usize {
        self.values.size() as _
    }
    pub fn new(vm: &mut Runtime, this: JsValue, size: usize) -> Self {
        let stack = vm.shadowstack();
        crate::root!(
            arr = stack,
            ArrayStorage::with_size(vm, size as _, size as _)
        );
        for i in 0..size {
            *arr.at_mut(i as _) = JsValue::encode_undefined_value();
        }
        Self {
            this,
            values: *arr,
            ctor_call: false,
        }
    }
    pub fn at_mut(&mut self, x: usize) -> &mut JsValue {
        if x < self.size() {
            self.values.at_mut(x as _)
        } else {
            panic!("Out of bounds arguments");
        }
    }
    pub fn at(&self, x: usize) -> JsValue {
        if x < self.size() {
            *self.values.at(x as _)
        } else {
            JsValue::encode_undefined_value()
        }
    }
}

impl GcCell for Arguments {
    fn deser_pair(&self) -> (usize, usize) {
        panic!("unserializable")
    }
    vtable_impl!();
}
unsafe impl Trace for Arguments {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.this.trace(tracer);
        self.values.trace(tracer);
    }
}

pub struct JsArguments {
    // TODO: Better alternative?
    pub mapping: Box<[Symbol]>,
    pub env: GcPointer<JsObject>,
}
#[allow(non_snake_case)]
impl JsArguments {
    define_jsclass!(JsArguments, Arguments);
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        desc: &PropertyDescriptor,
        _slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        match obj.define_own_indexed_property_internal(vm, index, desc, throwable) {
            Ok(false) | Err(_) => {
                if throwable {
                    let msg = JsString::new(vm, "[[DefineOwnProperty]] failed");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        vm, msg, None,
                    )));
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
                                    Env {
                                        record: arg.env.clone(),
                                    }
                                    .set_variable(
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
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if !JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot) {
            return false;
        }
        let arg = obj.as_arguments_mut();

        if arg.mapping.len() > index as usize {
            let mapped = arg.mapping[index as usize];
            if mapped != DUMMY_SYMBOL {
                let val = arg
                    .env
                    .get(vm, mapped)
                    .unwrap_or_else(|_| JsValue::encode_undefined_value());
                let attrs = slot.attributes();
                slot.set(val, attrs);
                return true;
            }
        }
        true
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let v = JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot);
        if name == "caller".intern() {
            match v {
                Ok(x) if x.is_callable() => {
                    if x.get_object()
                        .downcast::<JsObject>()
                        .unwrap()
                        .as_function()
                        .is_strict()
                    {
                        let msg =
                            JsString::new(vm, "access to strict function 'caller' not allowed");
                        return Err(JsValue::encode_object_value(JsTypeError::new(
                            vm, msg, None,
                        )));
                    }
                }
                _ => (),
            }
        }
        v
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        //!();
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn new(
        vm: &mut Runtime,
        env: GcPointer<JsObject>,
        params: &[Symbol],
        len: u32,
    ) -> GcPointer<JsObject> {
        root!(
            struct_ = vm.shadowstack(),
            vm.global_data().normal_arguments_structure.clone().unwrap()
        );
        let mut obj = JsObject::new(
            vm,
            &struct_,
            JsArguments::get_class(),
            ObjectTag::NormalArguments,
        );

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
                    JsValue::encode_undefined_value(),
                    create_data(AttrExternal::new(Some(W | C | E))).raw(),
                ),
                &mut slot,
                false,
            );

            mapping.push(*param);

            //let _ = obj.put(vm, Symbol::Indexed(i as _), JsValue::undefined(), false);
        }
        let _ = obj.put(
            vm,
            "length".intern(),
            JsValue::encode_f64_value(len as _),
            false,
        );
        obj.as_arguments_mut().mapping = mapping.into_boxed_slice();
        obj
    }
}

unsafe impl Trace for JsArguments {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.env.trace(tracer);
    }
}
