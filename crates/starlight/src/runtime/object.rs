use std::{mem::size_of, mem::ManuallyDrop};

use super::{
    arguments::Arguments,
    attributes::*,
    class::Class,
    error::JsTypeError,
    function::JsFunction,
    global::JsGlobal,
    indexed_elements::{IndexedElements, MAX_VECTOR_SIZE},
    js_arguments::JsArguments,
    number::JsNumber,
    property_descriptor::{DataDescriptor, PropertyDescriptor, StoredSlot},
    slot::*,
    storage::FixedStorage,
    string::*,
    structure::Structure,
    symbol::*,
};
use super::{method_table::MethodTable, value::JsValue};
use crate::{
    define_jsclass,
    gc::cell::{Cell, Gc, Trace, Tracer},
    gc::handle::Handle,
    heap::addr::Address,
    vm::*,
};
use std::collections::hash_map::Entry;

use wtf_rs::object_offsetof;

pub const OBJ_FLAG_TUPLE: u32 = 0x4;
pub const OBJ_FLAG_CALLABLE: u32 = 0x2;
pub const OBJ_FLAG_EXTENSIBLE: u32 = 0x1;

pub type ObjectSlots = FixedStorage<JsValue>;

#[repr(C)]
pub struct JsObject {
    tag: ObjectTag,
    class: &'static Class,
    structure: Gc<Structure>,
    slots: ObjectSlots,
    pub(crate) elements: IndexedElements,
    flags: u32,

    pub(crate) data_start: [ObjectData; 0],
}

impl JsObject {
    pub fn class(&self) -> &'static Class {
        self.class
    }
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn data<T>(&self) -> &mut ManuallyDrop<T> {
        unsafe {
            &mut *Address::from_ptr(self)
                .offset(object_offsetof!(Self, data_start))
                .to_mut_ptr::<_>()
        }
    }
    pub fn data_offset() -> usize {
        object_offsetof!(Self, data_start)
    }

    pub fn tag_offset() -> usize {
        object_offsetof!(Self, tag)
    }

    pub fn class_offsetof() -> usize {
        object_offsetof!(Self, class)
    }

    pub fn structure_offsetof() -> usize {
        object_offsetof!(Self, structure)
    }

    pub fn slots_offsetof() -> usize {
        object_offsetof!(Self, slots)
    }

    pub fn elements_offsetof() -> usize {
        object_offsetof!(Self, elements)
    }

    pub fn flags_offsetof() -> usize {
        object_offsetof!(Self, flags)
    }
    pub fn direct(&self, n: usize) -> &JsValue {
        &self.slots[n]
    }

    pub fn direct_mut(&mut self, n: usize) -> &mut JsValue {
        &mut self.slots[n]
    }
}

impl Drop for JsObject {
    fn drop(&mut self) {
        match self.tag {
            ObjectTag::Function => unsafe { ManuallyDrop::drop(&mut self.data::<JsFunction>()) },
            ObjectTag::Global => unsafe { ManuallyDrop::drop(&mut self.data::<JsGlobal>()) },
            ObjectTag::NormalArguments => unsafe {
                ManuallyDrop::drop(&mut self.data::<JsArguments>())
            },
            _ => (),
        }
    }
}

#[repr(C)]
pub union ObjectData {
    pub ordinary: (),
    pub global: ManuallyDrop<JsGlobal>,
    pub function: ManuallyDrop<JsFunction>,
    pub arguments: ManuallyDrop<JsArguments>,
}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for JsObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("JsObject", 5)?;
        x.serialize_field("tag", &format!("{:?}", self.tag));
        x.serialize_field("elements", &self.elements);
        x.serialize_field("slots", &self.slots.data);
        x.serialize_field("extensible", &self.is_extensible());
        x.serialize_field("callable", &self.is_callable());
        x.end()
    }
}
impl Cell for JsObject {
    fn compute_size(&self) -> usize {
        object_size_with_tag(self.tag)
    }
    fn set_class_value(&mut self, _class: &'static Class) {
        self.class = _class;
    }
    fn get_class_value(&self) -> Option<&'static Class> {
        Some(self.class)
    }

    fn get_structure(&self) -> Option<Gc<Structure>> {
        Some(self.structure)
    }
    fn set_structure(&mut self, _vm: &mut VirtualMachine, _structure: Gc<Structure>) {
        self.structure = _structure;
    }
}
unsafe impl Trace for JsObject {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.structure.trace(tracer);
        self.elements.trace(tracer);
        self.slots.trace(tracer);

        match self.tag {
            ObjectTag::Global => self.as_global().trace(tracer),
            ObjectTag::Function => self.as_function().trace(tracer),
            ObjectTag::String => self.as_string().value().trace(tracer),
            ObjectTag::NormalArguments => self.as_arguments().trace(tracer),
            _ => (),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EnumerationMode {
    Default,
    IncludeNotEnumerable,
}
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum JsHint {
    String,
    Number,
    None,
}

fn is_absent_descriptor(desc: &PropertyDescriptor) -> bool {
    if !desc.is_enumerable() && !desc.is_enumerable_absent() {
        return false;
    }

    if !desc.is_configurable() && !desc.is_configurable_absent() {
        return false;
    }
    if desc.is_accessor() {
        return false;
    }
    if desc.is_data() {
        return DataDescriptor { parent: *desc }.is_writable()
            && DataDescriptor { parent: *desc }.is_writable_absent();
    }
    true
}

#[allow(non_snake_case)]
impl JsObject {
    pub fn prototype(&self) -> Option<Gc<JsObject>> {
        self.structure.prototype()
    }

    pub fn GetNonIndexedPropertySlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        loop {
            if obj.get_own_non_indexed_property_slot(vm, name, slot) {
                break true;
            }
            match obj.prototype() {
                Some(proto) => obj = proto,
                _ => break false,
            }
        }
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let entry = obj.structure.get(vm, name);

        if !entry.is_not_found() {
            slot.set_woffset(
                *obj.direct(entry.offset as _),
                entry.attrs as _,
                Some(obj.as_dyn()),
                entry.offset,
            );
            return true;
        }
        false
    }

    pub fn PutNonIndexedSlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if !obj.can_put(vm, name, slot) {
            if throwable {
                todo!();
            }

            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Gc::ptr_eq(*base, obj) && slot.attributes().is_data() {
                    obj.define_own_non_indexed_property_slot(
                        vm,
                        name,
                        &*DataDescriptor::new(
                            val,
                            UNDEF_ENUMERABLE | UNDEF_CONFIGURABLE | UNDEF_WRITABLE,
                        ),
                        slot,
                        throwable,
                    )?;
                    return Ok(());
                }
            }

            if slot.attributes().is_accessor() {
                let ac = slot.accessor();
                let args = Arguments::new(vm, JsValue::new(obj), 1);
                let mut args = Handle::new(vm.space(), args);

                *args.at_mut(0) = val;
                return ac
                    .setter()
                    .as_cell()
                    .downcast::<JsObject>()
                    .unwrap()
                    .as_function_mut()
                    .call(vm, &mut args)
                    .map(|_| ());
            }
        }
        obj.define_own_non_indexed_property_slot(
            vm,
            name,
            &*DataDescriptor::new(val, W | C | E),
            slot,
            throwable,
        )?;

        Ok(())
    }

    pub fn GetOwnIndexedPropertySlotMethod(
        obj: Gc<Self>,
        _vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if obj.elements.dense() && index < obj.elements.vector.len() as u32 {
            let value = obj.elements.vector[index as usize];
            if value.is_empty() {
                return false;
            }

            slot.set_1(value, object_data(), Some(obj.as_dyn()));
            return true;
        }
        if let Some(map) = obj.elements.map {
            if index < obj.elements.length() {
                let it = map.get(&index);
                if let Some(it) = it {
                    slot.set_from_slot(it, Some(obj.as_dyn()));
                    return true;
                }
            }
        }

        false
    }

    pub fn PutIndexedSlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if index < MAX_VECTOR_SIZE as u32
            && obj.elements.dense()
            && obj.class.method_table.GetOwnIndexedPropertySlot as usize
                == Self::GetOwnIndexedPropertySlotMethod as usize
            && (obj.prototype().is_none()
                || obj.prototype().as_ref().unwrap().has_indexed_property())
        {
            slot.mark_put_result(PutResultType::IndexedOptimized, index);
            obj.define_own_indexe_value_dense_internal(vm, index, val, false);

            return Ok(());
        }
        if !obj.can_put_indexed(vm, index, slot) {
            if throwable {
                todo!()
            }
            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Gc::ptr_eq(*base, obj) && slot.attributes().is_data() {
                    obj.define_own_indexed_property_slot(
                        vm,
                        index,
                        &*DataDescriptor::new(
                            val,
                            UNDEF_ENUMERABLE | UNDEF_CONFIGURABLE | UNDEF_WRITABLE,
                        ),
                        slot,
                        throwable,
                    )?;
                    return Ok(());
                }
            }

            if slot.attributes().is_accessor() {
                let ac = slot.accessor();
                let args = Arguments::new(vm, JsValue::new(obj), 1);
                let mut args = Handle::new(vm.space(), args);

                *args.at_mut(0) = val;
                return ac
                    .setter()
                    .as_cell()
                    .downcast::<JsObject>()
                    .unwrap()
                    .as_function_mut()
                    .call(vm, &mut args)
                    .map(|_| ());
            }
        }

        obj.define_own_indexed_property_slot(
            vm,
            index,
            &*DataDescriptor::new(val, W | E | C),
            slot,
            throwable,
        )?;
        Ok(())
    }

    pub fn GetIndexedPropertySlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        loop {
            if obj.get_own_indexed_property_slot(vm, index, slot) {
                return true;
            }

            match obj.prototype() {
                Some(proto) => obj = proto,
                None => break false,
            }
        }
    }

    pub fn is_extensible(&self) -> bool {
        (self.flags & OBJ_FLAG_EXTENSIBLE) != 0
    }

    pub fn set_callable(&mut self, val: bool) {
        if val {
            self.flags |= OBJ_FLAG_CALLABLE;
        } else {
            self.flags &= !OBJ_FLAG_CALLABLE;
        }
    }

    pub fn is_callable(&self) -> bool {
        (self.flags & OBJ_FLAG_CALLABLE) != 0
            || self.class as *const Class == JsFunction::get_class() as *const Class
    }

    // section 8.12.9 `[[DefineOwnProperty]]`
    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut obj = obj.root(vm.space());
        if !slot.is_used() {
            obj.get_own_property_slot(vm, name, slot);
        }

        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Gc::ptr_eq(*base, *obj) {
                    let mut returned = false;
                    if slot.is_defined_property_accepted(vm, desc, throwable, &mut returned)? {
                        if slot.has_offset() {
                            let old = slot.attributes();
                            slot.merge(vm, desc);
                            if old != slot.attributes() {
                                let new_struct = obj.structure.change_attributes_transition(
                                    vm,
                                    name,
                                    slot.attributes(),
                                );
                                obj.set_structure(vm, new_struct);
                            }
                            *obj.direct_mut(slot.offset() as _) = slot.value();

                            slot.mark_put_result(PutResultType::Replace, slot.offset());
                        } else {
                            let mut offset = 0;
                            slot.merge(vm, desc);
                            let new_struct = obj.structure.add_property_transition(
                                vm,
                                name,
                                slot.attributes(),
                                &mut offset,
                            );
                            obj.set_structure(vm, new_struct);
                            let s = obj.structure;
                            //   println!("resize to {} from {}", s.get_slots_size(), obj.slots.size());
                            obj.slots.resize(vm, s.get_slots_size(), JsValue::empty());

                            *obj.direct_mut(offset as _) = slot.value();
                            slot.mark_put_result(PutResultType::New, offset);
                        }
                    }
                    return Ok(returned);
                }
            }
        }

        if !obj.is_extensible() {
            if throwable {
                todo!();
            }

            return Ok(false);
        }

        let mut offset = 0;
        let stored = StoredSlot::new(vm, desc);
        let s = obj
            .structure
            .add_property_transition(vm, name, stored.attributes(), &mut offset);
        obj.structure = s;

        let s = obj.structure;

        obj.slots.resize(vm, s.get_slots_size(), JsValue::empty());
        assert!(stored.value() == desc.value());
        *obj.direct_mut(offset as _) = stored.value();
        slot.mark_put_result(PutResultType::New, offset);
        //println!("add");
        Ok(true)
    }

    pub fn DefineOwnIndexedPropertySlotMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if obj.class.method_table.GetOwnIndexedPropertySlot as usize
            != Self::GetOwnIndexedPropertySlotMethod as usize
        {
            // We should reject following case
            //   var str = new String('str');
            //   Object.defineProperty(str, '0', { value: 0 });
            if !slot.is_used() {
                obj.get_own_indexed_property_slot(vm, index, slot);
            }

            let mut returned = false;
            if !slot.is_not_found() {
                if let Some(base) = slot.base() {
                    if Gc::ptr_eq(*base, obj) {
                        if !slot.is_defined_property_accepted(vm, desc, throwable, &mut returned)? {
                            return Ok(returned);
                        }
                    }
                }
            }
        }

        obj.define_own_indexed_property_internal(vm, index, desc, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_non_indexed_property_slot(vm, name, slot) {
            return slot.get(vm, JsValue::new(obj));
        }
        Ok(JsValue::undefined())
    }
    pub fn GetIndexedSlotMethod(
        obj: Gc<Self>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_indexed_property_slot(vm, index, slot) {
            return slot.get(vm, JsValue::new(obj));
        }

        Ok(JsValue::undefined())
    }

    pub fn DeleteNonIndexedMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        if !obj.get_own_property_slot(vm, name, &mut slot) {
            return Ok(true);
        }

        if !slot.attributes().is_configurable() {
            if throwable {
                todo!();
            }
            return Ok(false);
        }

        let offset = if slot.has_offset() {
            slot.offset()
        } else {
            let entry = obj.structure.get(vm, name);
            if entry.is_not_found() {
                return Ok(true);
            }
            entry.offset
        };

        let s = obj.structure.delete_property_transition(vm, name);
        obj.structure = s;
        *obj.direct_mut(offset as _) = JsValue::empty();
        Ok(true)
    }

    #[allow(clippy::unnecessary_unwrap)]
    pub fn delete_indexed_internal(
        &mut self,
        _vm: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if self.elements.length() <= index {
            return Ok(true);
        }

        if self.elements.dense() {
            if index < self.elements.vector.len() as u32 {
                self.elements.vector[index as usize] = JsValue::empty();
                return Ok(true);
            }

            if index < MAX_VECTOR_SIZE as u32 {
                return Ok(true);
            }
        }

        if self.elements.map.is_none() {
            return Ok(true);
        }
        let mut map = self.elements.map.unwrap();

        match map.entry(index) {
            Entry::Vacant(_) => Ok(true),
            Entry::Occupied(x) => {
                if !x.get().attributes().is_configurable() {
                    if throwable {
                        let msg = JsString::new(_vm, "trying to delete non-configurable property");
                        return Err(JsValue::new(JsTypeError::new(_vm, msg, None)));
                    }
                    return Ok(false);
                }
                x.remove();
                if map.is_empty() {
                    self.elements.make_dense();
                }
                Ok(true)
            }
        }
    }
    pub fn DeleteIndexedMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if obj.class.method_table.GetOwnIndexedPropertySlot as usize
            == Self::GetOwnIndexedPropertySlotMethod as usize
        {
            return obj.delete_indexed_internal(vm, index, throwable);
        }
        let mut slot = Slot::new();
        if !(obj.class.method_table.GetOwnIndexedPropertySlot)(obj, vm, index, &mut slot) {
            return Ok(true);
        }

        if !slot.attributes().is_configurable() {
            if throwable {
                todo!();
            }
            return Ok(false);
        }

        obj.delete_indexed_internal(vm, index, throwable)
    }
    #[allow(unused_variables)]
    pub fn GetPropertyNamesMethod(
        obj: Gc<Self>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        obj.get_own_property_names(vm, collector, mode);
        let mut obj = obj.prototype();
        while let Some(proto) = obj {
            proto.get_own_property_names(vm, collector, mode);
            obj = proto.prototype();
        }
    }
    #[allow(unused_variables)]
    pub fn GetOwnPropertyNamesMethod(
        mut obj: Gc<Self>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        if obj.elements.dense() {
            for index in 0..obj.elements.vector.len() {
                let it = obj.elements.vector[index];
                if !it.is_empty() {
                    collector(Symbol::Indexed(index as _), u32::MAX);
                }
            }
        }

        if let Some(map) = obj.elements.map {
            for it in map.iter() {
                if mode == EnumerationMode::IncludeNotEnumerable
                    || it.1.attributes().is_enumerable()
                {
                    collector(Symbol::Indexed(*it.0), u32::MAX);
                }
            }
        }

        obj.structure.get_own_property_names(
            vm,
            mode == EnumerationMode::IncludeNotEnumerable,
            collector,
        );
    }

    /// 7.1.1 ToPrimitive
    ///
    ///
    /// 7.1.1.1 OrdinaryToPrimitive
    #[allow(unused_variables)]
    pub fn DefaultValueMethod(
        obj: Gc<Self>,
        vm: &mut VirtualMachine,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        let args = Arguments::new(vm, JsValue::new(obj), 0);
        let mut args = Handle::new(vm.space(), args);

        macro_rules! try_ {
            ($sym: expr) => {
                let try_get = vm.description($sym);

                let m = obj.get(vm, $sym)?;

                if m.is_callable() {
                    let res = m
                        .as_cell()
                        .downcast::<JsObject>()
                        .unwrap()
                        .as_function_mut()
                        .call(vm, &mut args)?;
                    if res.is_primitive() || res.is_undefined_or_null() {
                        return Ok(res);
                    }
                }
            };
        }

        if hint == JsHint::String {
            try_!(Symbol::toString());
            try_!(Symbol::valueOf());
        } else {
            try_!(Symbol::valueOf());
            try_!(Symbol::toString());
        }

        let msg = JsString::new(vm, "invalid default value");
        Err(JsValue::new(JsTypeError::new(vm, msg, None)))
    }
    /*const fn get_method_table() -> MethodTable {
        js_method_table!(JsObject)
    }*/

    define_jsclass!(JsObject, Object);
    pub fn new_empty(vm: &mut VirtualMachine) -> Gc<Self> {
        let structure = vm.global_data().empty_object_struct.unwrap();
        Self::new(vm, structure, Self::get_class(), ObjectTag::Ordinary)
    }
    pub fn new(
        vm: &mut VirtualMachine,
        structure: Gc<Structure>,
        class: &'static Class,
        tag: ObjectTag,
    ) -> Gc<Self> {
        let this = Self {
            structure,
            class,

            slots: FixedStorage::with_capacity(vm, structure.get_slots_size(), JsValue::empty()),
            data_start: [],
            elements: IndexedElements::new(vm),
            flags: OBJ_FLAG_EXTENSIBLE,
            tag,
        };
        vm.space().alloc(this)
    }

    pub fn tag(&self) -> ObjectTag {
        self.tag
    }
    pub fn as_global(&self) -> &JsGlobal {
        assert_eq!(self.tag, ObjectTag::Global);
        unsafe { &*self.data::<JsGlobal>() }
    }
    pub fn as_global_mut(&mut self) -> &mut JsGlobal {
        assert_eq!(self.tag, ObjectTag::Global);
        unsafe { &mut **self.data::<JsGlobal>() }
    }
    pub fn as_function(&self) -> &JsFunction {
        assert_eq!(self.tag, ObjectTag::Function);
        unsafe { &*self.data::<JsFunction>() }
    }
    pub fn as_function_mut(&mut self) -> &mut JsFunction {
        assert_eq!(self.tag, ObjectTag::Function);
        unsafe { &mut *self.data::<JsFunction>() }
    }

    pub fn as_arguments(&self) -> &JsArguments {
        assert_eq!(self.tag, ObjectTag::NormalArguments);
        unsafe { &*self.data::<JsArguments>() }
    }
    pub fn as_arguments_mut(&mut self) -> &mut JsArguments {
        assert_eq!(self.tag, ObjectTag::NormalArguments);
        unsafe { &mut *self.data::<JsArguments>() }
    }

    pub fn as_number(&self) -> &JsNumber {
        assert_eq!(self.tag, ObjectTag::Number);
        unsafe { &*self.data::<JsNumber>() }
    }

    pub fn as_number_mut(&mut self) -> &mut JsNumber {
        assert_eq!(self.tag, ObjectTag::Number);
        unsafe { &mut *self.data::<JsNumber>() }
    }

    pub fn as_string(&self) -> &JsStringObject {
        assert_eq!(self.tag, ObjectTag::String);
        unsafe { &*self.data::<JsStringObject>() }
    }

    pub fn as_string_mut(&mut self) -> &mut JsStringObject {
        assert_eq!(self.tag, ObjectTag::String);
        unsafe { &mut *self.data::<JsStringObject>() }
    }
}

impl Gc<JsObject> {
    pub fn get_own_property_names(
        &self,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        (self.class.method_table.GetOwnPropertyNames)(*self, vm, collector, mode)
    }
    pub fn get_property_names(
        &self,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        (self.class.method_table.GetPropertyNames)(*self, vm, collector, mode)
    }
    pub fn put_non_indexed_slot(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        unsafe {
            (self.class.method_table.PutNonIndexedSlot)(*self, vm, name, val, slot, throwable)
        }
    }
    #[allow(clippy::wrong_self_convention)]
    /// 7.1 Type Conversion
    ///
    /// 7.1.1 ToPrimitive
    pub fn to_primitive(
        &mut self,
        vm: &mut VirtualMachine,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        let exotic_to_prim = self.get_method(vm, Symbol::toPrimitive());

        let obj = *self;
        match exotic_to_prim {
            Ok(val) => {
                // downcast_unchecked here is safe because `get_method` returns `Err` if property is not a function.
                let mut func = unsafe { val.as_cell().downcast_unchecked::<JsObject>() };
                let f = func.as_function_mut();
                let args = Arguments::new(vm, JsValue::new(obj), 1);
                let mut args = Handle::new(vm.space(), args);
                *args.at_mut(0) = match hint {
                    JsHint::Number | JsHint::None => JsValue::new(JsString::new(vm, "number")),
                    JsHint::String => JsValue::new(JsString::new(vm, "string")),
                };

                f.call(vm, &mut args)
            }
            _ => (self.class.method_table.DefaultValue)(obj, vm, hint),
        }
    }
    pub fn delete_non_indexed(
        &mut self,
        ctx: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DeleteNonIndexed)(*self, ctx, name, throwable)
    }
    pub fn delete(
        &mut self,
        ctx: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        match name {
            Symbol::Indexed(index) => self.delete_indexed(ctx, index, throwable),
            name => self.delete_non_indexed(ctx, name, throwable),
        }
    }
    pub fn delete_indexed(
        &mut self,
        ctx: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DeleteIndexed)(*self, ctx, index, throwable)
    }

    pub fn has_indexed_property(&self) -> bool {
        let mut obj = *self;
        loop {
            if obj.structure.is_indexed() {
                return true;
            }
            match obj.prototype() {
                Some(proto) => obj = proto,
                None => break false,
            }
        }
    }
    pub fn get_non_indexed_property_slot(
        &self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        unsafe { (self.class.method_table.GetNonIndexedPropertySlot)(*self, vm, name, slot) }
    }
    pub fn get_indexed_property_slot(
        &self,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetIndexedPropertySlot)(*self, vm, index, slot)
    }

    pub fn get_own_non_indexed_property_slot(
        &self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        /*let mut structure = self.structure;
        let entry = structure.get(vm, name);
        if !entry.is_not_found() {
            slot.set_1(
                *self.direct(entry.offset as _),
                entry.attrs as _,
                Some(unsafe { Heap::<JsObject>::from_raw(self).as_dyn() }),
            );
            return true;
        }
        false*/
        (self.class.method_table.GetOwnNonIndexedPropertySlot)(*self, vm, name, slot)
    }
    pub fn can_put(&self, vm: &mut VirtualMachine, name: Symbol, slot: &mut Slot) -> bool {
        if let Symbol::Indexed(index) = name {
            self.can_put_indexed(vm, index, slot)
        } else {
            self.can_put_non_indexed(vm, name, slot)
        }
    }

    pub fn can_put_indexed(&self, vm: &mut VirtualMachine, index: u32, slot: &mut Slot) -> bool {
        if self.get_indexed_property_slot(vm, index, slot) {
            if slot.attributes().is_accessor() {
                return slot.accessor().setter().is_cell() && !slot.accessor().setter().is_empty();
            } else {
                return slot.attributes().is_writable();
            }
        }
        self.is_extensible()
    }
    pub fn put_indexed_slot(
        &mut self,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        unsafe { (self.class.method_table.PutIndexedSlot)(*self, vm, index, val, slot, throwable) }
    }
    pub fn put_slot(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if let Symbol::Indexed(index) = name {
            self.put_indexed_slot(vm, index, val, slot, throwable)
        } else {
            self.put_non_indexed_slot(vm, name, val, slot, throwable)
        }
    }
    pub fn structure(&self) -> Gc<Structure> {
        self.structure
    }
    pub fn get_property_slot(
        &self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if let Symbol::Indexed(index) = name {
            self.get_indexed_property_slot(vm, index, slot)
        } else {
            self.get_non_indexed_property_slot(vm, name, slot)
        }
    }

    pub fn get_property(&self, vm: &mut VirtualMachine, name: Symbol) -> PropertyDescriptor {
        let mut slot = Slot::new();
        if self.get_property_slot(vm, name, &mut slot) {
            return slot.to_descriptor();
        }
        PropertyDescriptor::new_val(JsValue::empty(), AttrSafe::not_found())
    }
    pub fn get_method(&self, vm: &mut VirtualMachine, name: Symbol) -> Result<JsValue, JsValue> {
        let val = self.get(vm, name);
        match val {
            Err(e) => Err(e),
            Ok(val) => {
                if val.is_callable() {
                    return Ok(val);
                } else {
                    let desc = vm.description(name);
                    let msg = JsString::new(vm, format!("Property '{}' is not a method", desc));
                    Err(JsValue::new(JsTypeError::new(vm, msg, None)))
                }
            }
        }
    }
    pub fn put(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        throwable: bool,
    ) -> Result<(), JsValue> {
        let mut slot = Slot::new();

        self.put_slot(vm, name, val, &mut slot, throwable)
    }

    pub fn can_put_non_indexed(
        &self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if self.get_non_indexed_property_slot(vm, name, slot) {
            if slot.attributes().is_accessor() {
                if slot.attributes().is_accessor() {
                    return slot.accessor().setter().is_cell()
                        && !slot.accessor().setter().is_empty();
                } else {
                    return slot.attributes().is_writable();
                }
            }
        }
        self.is_extensible()
    }

    pub fn has_property(&self, ctx: &mut VirtualMachine, name: Symbol) -> bool {
        let mut slot = Slot::new();
        self.get_property_slot(ctx, name, &mut slot)
    }
    pub fn has_own_property(&self, ctx: &mut VirtualMachine, name: Symbol) -> bool {
        let mut slot = Slot::new();
        self.get_own_property_slot(ctx, name, &mut slot)
    }
    pub fn define_own_indexed_property_slot(
        &mut self,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DefineOwnIndexedPropertySlot)(
            *self, vm, index, desc, slot, throwable,
        )
    }
    pub fn get_own_property_slot(
        &self,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if let Symbol::Indexed(index) = name {
            self.get_own_indexed_property_slot(vm, index, slot)
        } else {
            self.get_own_non_indexed_property_slot(vm, name, slot)
        }
    }
    pub fn get(&self, ctx: &mut VirtualMachine, name: Symbol) -> Result<JsValue, JsValue> {
        let mut slot = Slot::new();
        self.get_slot(ctx, name, &mut slot)
    }
    pub fn get_slot(
        &self,
        ctx: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if let Symbol::Indexed(index) = name {
            self.get_indexed_slot(ctx, index, slot)
        } else {
            self.get_non_indexed_slot(ctx, name, slot)
        }
    }

    pub fn get_indexed_slot(
        &self,
        ctx: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        (self.class.method_table.GetIndexedSlot)(unsafe { *self }, ctx, index, slot)
    }

    pub fn get_non_indexed_slot(
        &self,
        ctx: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        (self.class.method_table.GetNonIndexedSlot)(unsafe { *self }, ctx, name, slot)
    }
    pub fn get_own_indexed_property_slot(
        &self,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetOwnIndexedPropertySlot)(*self, vm, index, slot)
        //unsafe { JsObject::GetOwnIndexedPropertySlotMethod(*self, vm, index, slot) }
    }
    fn define_own_indexe_value_dense_internal(
        &mut self,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        absent: bool,
    ) {
        if index < self.elements.vector.len() as u32 {
            if !absent {
                self.elements.vector[index as usize] = val;
            } else {
                self.elements.vector[index as usize] = JsValue::undefined();
            }
        } else {
            if !self.structure.is_indexed() {
                let s = self
                    .structure
                    .root(vm.space())
                    .change_indexed_transition(vm);

                self.set_structure(vm, s)
            }

            self.elements
                .vector
                .resize(vm, index as usize + 1, JsValue::empty());

            if !absent {
                self.elements.vector[index as usize] = val;
            } else {
                self.elements.vector[index as usize] = JsValue::undefined();
            }
        }
        if index >= self.elements.length() {
            self.elements.set_length(index + 1);
        }
    }
    pub fn define_own_indexed_property_internal(
        &mut self,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if index >= self.elements.length() && !self.elements.writable() {
            if throwable {
                todo!()
            }
            return Ok(false);
        }

        if self.elements.dense() {
            if desc.is_default() {
                if index < MAX_VECTOR_SIZE as u32 {
                    self.define_own_indexe_value_dense_internal(
                        vm,
                        index,
                        desc.value(),
                        desc.is_value_absent(),
                    );
                    return Ok(true);
                }
            } else {
                if is_absent_descriptor(desc) {
                    if index < self.elements.vector.len() as u32
                        && !self.elements.vector[index as usize].is_empty()
                    {
                        if !desc.is_value_absent() {
                            self.elements.vector[index as usize] = desc.value();
                        }
                        return Ok(true);
                    }
                }

                if index < MAX_VECTOR_SIZE as u32 {
                    self.elements.make_sparse(vm);
                }
            }
        }

        let mut sparse = self.elements.ensure_map(vm);
        match sparse.get_mut(&index) {
            Some(entry) => {
                let mut returned = false;
                if entry.is_defined_property_accepted(vm, desc, throwable, &mut returned)? {
                    entry.merge(vm, desc);
                }
                Ok(returned)
            }
            None if !self.is_extensible() => {
                if throwable {
                    todo!()
                }
                Ok(false)
            }
            None => {
                if !self.structure.is_indexed() {
                    let s = self.structure.change_indexed_transition(vm);
                    self.structure = s;
                }
                if index >= self.elements.length() {
                    self.elements.set_length(index + 1);
                }
                sparse.insert(index, StoredSlot::new(vm, desc));
                Ok(true)
            }
        }
    }

    pub fn define_own_property_slot(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if let Symbol::Indexed(index) = name {
            self.define_own_indexed_property_internal(vm, index, desc, throwable)
        } else {
            self.define_own_non_indexed_property_slot(vm, name, desc, slot, throwable)
        }
    }
    pub fn define_own_non_indexed_property_slot(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        unsafe {
            (self.class.method_table.DefineOwnNonIndexedPropertySlot)(
                *self, vm, name, desc, slot, throwable,
            )
        }
    }
    pub fn define_own_property(
        &mut self,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        self.define_own_property_slot(vm, name, desc, &mut slot, throwable)
    }
}

pub fn object_size_with_tag(tag: ObjectTag) -> usize {
    let size = size_of::<JsObject>();
    match tag {
        ObjectTag::Global => size + size_of::<JsGlobal>(),
        ObjectTag::NormalArguments => size + size_of::<JsArguments>(),
        ObjectTag::Function => size + size_of::<JsFunction>(),
        _ => size,
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ObjectTag {
    Ordinary,
    Array,
    Set,
    String,
    Map,
    Number,
    Error,
    Global,
    Json,
    Function,
    Regex,
    ArrayBuffer,
    Int8Array,
    Uint8Array,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Int64Array,
    Uint64Array,
    Float32Array,
    Float64Array,
    Uint8ClampedArray,
    Reflect,
    Iterator,
    ArrayIterator,
    MapIterator,
    SetIterator,
    StringIterator,
    ForInIterator,
    WeakMap,
    WeakSet,

    NormalArguments,
    StrictArguments,

    Proxy,
}

#[cfg(test)]
mod tests {
    use crate::runtime::array::JsArray;

    use super::*;
    use wtf_rs::keep_on_stack;
    #[test]
    fn test_put() {
        let mut vm = VirtualMachine::new(Options::default());

        {
            let my_struct = Structure::new_indexed(&mut vm, None, true);
            let obj = JsObject::new(&mut vm, my_struct, JsObject::get_class(), ObjectTag::Array);
            let mut obj = Handle::new(vm.space(), obj);
            keep_on_stack!(&obj, &my_struct);
            let key = vm.intern("foo");
            let put = obj.put(&mut vm, key, JsValue::new(42), false);
            assert!(put.is_ok());
            vm.space().gc();
            let val = obj.get_property(&mut vm, key);
            assert!(val.is_data());
            assert!(val.value().is_int32());
            assert_eq!(val.value().as_int32(), 42);
        }

        VirtualMachineRef::dispose(vm);
    }

    #[test]
    fn test_put_indexed() {
        let mut vm = VirtualMachine::new(Options::default());
        {
            let struct_ = Structure::new_indexed(&mut vm, None, true);
            let my_struct = Handle::new(vm.space(), struct_);
            let obj = JsObject::new(&mut vm, *my_struct, JsArray::get_class(), ObjectTag::Array);
            let mut obj = Handle::new(vm.space(), obj);

            let key = vm.intern("foo");
            let key2 = Symbol::Indexed(0);
            let put = obj.put(&mut vm, key, JsValue::new(42), false);

            assert!(put.is_ok());
            let put = obj.put(&mut vm, key2, JsValue::new(42.5), false);
            assert!(put.is_ok());
            vm.space().gc();
            let val = obj.get_property(&mut vm, key);
            assert!(val.is_data());
            assert!(val.value().is_int32());
            assert_eq!(val.value().as_int32(), 42);
            assert_eq!(
                obj.get_property(&mut vm, Symbol::Indexed(0))
                    .value()
                    .as_double(),
                42.5
            );

            drop(obj);
            drop(my_struct);

            VirtualMachineRef::dispose(vm);
        }
    }
}
