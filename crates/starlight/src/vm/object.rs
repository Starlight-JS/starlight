/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::{
    arguments::*,
    array_storage::ArrayStorage,
    attributes::*,
    class::{Class, JsClass},
    error::*,
    function::*,
    global::JsGlobal,
    indexed_elements::IndexedElements,
    property_descriptor::StoredSlot,
    property_descriptor::{DataDescriptor, PropertyDescriptor},
    slot::*,
    string::*,
    structure::Structure,
    symbol_table::{Internable, Symbol},
    value::JsValue,
    Context, Runtime,
};
use super::{indexed_elements::MAX_VECTOR_SIZE, method_table::*};
use crate::prelude::*;
use crate::{gc::cell::GcPointerBase, vm::promise::JsPromise};
use crate::{
    gc::{
        cell::{GcCell, GcPointer, Trace, Tracer},
        snapshot::{deserializer::Deserializable, serializer::Serializable},
    },
    JsTryFrom,
};
use std::{
    collections::hash_map::Entry,
    intrinsics::{likely, transmute, unlikely},
    marker::PhantomData,
    mem::{size_of, ManuallyDrop},
    ops::{Deref, DerefMut},
};

use wtf_rs::object_offsetof;
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
pub const OBJ_FLAG_TUPLE: u32 = 0x4;
pub const OBJ_FLAG_CALLABLE: u32 = 0x2;
pub const OBJ_FLAG_EXTENSIBLE: u32 = 0x1;
pub type FixedStorage = GcPointer<ArrayStorage>;
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

#[repr(C)]
pub struct JsObject {
    pub(crate) tag: ObjectTag,
    pub(crate) class: &'static Class,
    pub(crate) structure: GcPointer<Structure>,
    pub(crate) indexed: IndexedElements,
    pub(crate) slots: FixedStorage,
    pub(crate) flags: u32,

    pub(crate) object_data_start: u8,
}
impl JsObject {
    pub fn direct(&self, n: usize) -> &JsValue {
        self.slots.at(n as _)
    }

    pub fn direct_mut(&mut self, n: usize) -> &mut JsValue {
        self.slots.at_mut(n as _)
    }

    pub fn is_class(&self, cls: &Class) -> bool {
        std::ptr::eq(self.class, cls)
    }

    #[allow(clippy::mut_from_ref)]
    pub fn data<T>(&self) -> &mut ManuallyDrop<T> {
        unsafe {
            &mut *(self as *const Self as *mut u8)
                .add(object_offsetof!(Self, object_data_start))
                .cast::<_>()
        }
    }
    pub fn as_function(&self) -> &JsFunction {
        assert_eq!(self.tag, ObjectTag::Function);
        &*self.data::<JsFunction>()
    }
    pub fn as_function_mut(&mut self) -> &mut JsFunction {
        assert_eq!(self.tag, ObjectTag::Function);

        &mut *self.data::<JsFunction>()
    }

    pub fn as_data<T>(&mut self) -> &mut T
    where
        T: JsClass,
    {
        assert!(self.is_class(T::class()));
        &mut *self.data::<T>()
    }

    pub fn as_promise(&self) -> &JsPromise {
        assert_eq!(self.tag, ObjectTag::Ordinary);
        assert!(self.is_class(JsPromise::class()));
        &*self.data::<JsPromise>()
    }
    pub fn as_promise_mut(&mut self) -> &mut JsPromise {
        assert_eq!(self.tag, ObjectTag::Ordinary);
        assert!(self.is_class(JsPromise::class()));
        &mut *self.data::<JsPromise>()
    }
    pub fn as_string_object(&self) -> &JsStringObject {
        assert_eq!(self.tag, ObjectTag::String);
        &*self.data::<JsStringObject>()
    }
    pub fn as_string_object_mut(&mut self) -> &mut JsStringObject {
        assert_eq!(self.tag, ObjectTag::String);

        &mut *self.data::<JsStringObject>()
    }
    pub fn as_global(&self) -> &JsGlobal {
        assert_eq!(self.tag, ObjectTag::Global);
        &*self.data::<JsGlobal>()
    }
    pub fn as_global_mut(&mut self) -> &mut JsGlobal {
        assert_eq!(self.tag, ObjectTag::Global);
        &mut **self.data::<JsGlobal>()
    }

    pub fn as_arguments(&self) -> &JsArguments {
        assert_eq!(self.tag, ObjectTag::NormalArguments);
        &*self.data::<JsArguments>()
    }
    pub fn as_arguments_mut(&mut self) -> &mut JsArguments {
        assert_eq!(self.tag, ObjectTag::NormalArguments);
        &mut *self.data::<JsArguments>()
    }
}
unsafe impl Trace for JsObject {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.structure.trace(visitor);
        self.slots.trace(visitor);
        self.indexed.trace(visitor);
        match self.tag {
            ObjectTag::Global => {
                self.as_global_mut().trace(visitor);
            }
            ObjectTag::NormalArguments => self.as_arguments_mut().trace(visitor),
            ObjectTag::Function => self.as_function_mut().trace(visitor),
            ObjectTag::String => self.as_string_object_mut().value.trace(visitor),
            _ => (),
        }
        if let Some(trace) = self.class.trace {
            trace(visitor, self);
        }
    }
}
impl GcCell for JsObject {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    fn compute_size(&self) -> usize {
        object_size_with_additional(self.class)
    }
}
impl Drop for JsObject {
    fn drop(&mut self) {
        match self.tag {
            ObjectTag::Global => unsafe {
                ManuallyDrop::drop(self.data::<JsGlobal>());
            },
            ObjectTag::Function => unsafe { ManuallyDrop::drop(self.data::<JsFunction>()) },
            ObjectTag::NormalArguments => unsafe { ManuallyDrop::drop(self.data::<JsArguments>()) },
            _ => (),
        }
        if let Some(drop_fn) = self.class.drop {
            drop_fn(unsafe {
                std::mem::transmute(self as *mut Self as usize - size_of::<GcPointerBase>())
            });
        }
    }
}

pub fn object_size_with_additional(cls: &Class) -> usize {
    size_of::<JsObject>()
        + if let Some(sz) = cls.additional_size {
            sz()
        } else {
            0
        }
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

impl JsClass for JsObject {
    fn class() -> &'static Class {
        define_jsclass!(JsObject, Object)
    }
}

#[allow(non_snake_case)]
impl JsObject {
    pub fn prototype(&self) -> Option<&GcPointer<JsObject>> {
        self.structure.prototype()
    }
    pub unsafe fn prototype_mut(&mut self) -> Option<&mut GcPointer<JsObject>> {
        self.structure.prototype_mut()
    }

    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let stack = ctx.shadowstack();
        letroot!(obj = stack, *obj);
        loop {
            if obj.get_own_non_indexed_property_slot(ctx, name, slot) {
                break true;
            }
            match obj.prototype() {
                Some(proto) => *obj = *proto,
                _ => break false,
            }
        }
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let entry = obj.structure.get(ctx, name);

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
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        let stack = ctx.shadowstack();
        if !obj.can_put(ctx, name, slot) {
            if throwable {
                let msg = JsString::new(ctx, "put failed");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }

            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if GcPointer::ptr_eq(base, obj) && slot.attributes().is_data() {
                    obj.define_own_non_indexed_property_slot(
                        ctx,
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
                letroot!(ac = stack, slot.accessor());
                let mut tmp = [JsValue::encode_undefined_value()];
                letroot!(
                    args = stack,
                    Arguments::new(JsValue::encode_object_value(*obj), &mut tmp)
                );
                *args.at_mut(0) = val;
                return ac
                    .setter()
                    .get_object()
                    .downcast::<JsObject>()
                    .unwrap()
                    .as_function_mut()
                    .call(ctx, &mut args, JsValue::encode_object_value(*obj))
                    .map(|_| ());
            }
        }
        obj.define_own_non_indexed_property_slot(
            ctx,
            name,
            &*DataDescriptor::new(val, W | C | E),
            slot,
            throwable,
        )?;

        Ok(())
    }

    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        _ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if obj.indexed.dense() && index < obj.indexed.vector.size() as u32 {
            let value = obj.indexed.vector.at(index);
            if value.is_empty() {
                return false;
            }

            slot.set_1(*value, object_data(), Some(obj.as_dyn()));
            return true;
        }
        if let Some(map) = obj.indexed.map.as_ref() {
            if index < obj.indexed.length() {
                let it = map.get(&index);
                if let Some(it) = it {
                    slot.set_from_slot(it, Some((*obj).as_dyn()));
                    return true;
                }
            }
        }

        false
    }

    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if index < MAX_VECTOR_SIZE as u32
            && obj.indexed.dense()
            && obj.class.method_table.GetOwnIndexedPropertySlot as usize
                == Self::GetOwnIndexedPropertySlotMethod as usize
            && (obj.prototype().is_none()
                || obj.prototype().as_ref().unwrap().has_indexed_property())
        {
            slot.mark_put_result(PutResultType::IndexedOptimized, index);
            obj.define_own_indexe_value_dense_internal(ctx, index, val, false);

            return Ok(());
        }
        let stack = ctx.shadowstack();
        if !obj.can_put_indexed(ctx, index, slot) {
            if throwable {
                let msg = JsString::new(ctx, "put failed");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if GcPointer::ptr_eq(base, obj) && slot.attributes().is_data() {
                    obj.define_own_indexed_property_slot(
                        ctx,
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
                letroot!(ac = stack, slot.accessor());
                let mut tmp = [JsValue::encode_undefined_value()];
                letroot!(
                    args = stack,
                    Arguments::new(JsValue::encode_object_value(*obj), &mut tmp,)
                );

                *args.at_mut(0) = val;
                return ac
                    .setter()
                    .get_object()
                    .downcast::<JsObject>()
                    .unwrap()
                    .as_function_mut()
                    .call(ctx, &mut args, ac.setter())
                    .map(|_| ());
            }
        }

        obj.define_own_indexed_property_slot(
            ctx,
            index,
            &*DataDescriptor::new(val, W | E | C),
            slot,
            throwable,
        )?;
        Ok(())
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        let stack = ctx.shadowstack();
        letroot!(obj = stack, *obj);
        loop {
            if obj.get_own_indexed_property_slot(ctx, index, slot) {
                return true;
            }

            match obj.prototype() {
                Some(proto) => *obj = *proto,
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
    }

    // section 8.12.9 `[[DefineOwnProperty]]`
    pub fn DefineOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        mut ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if !slot.is_used() {
            obj.get_own_property_slot(ctx, name, slot);
        }

        let stack = ctx.shadowstack();
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if GcPointer::ptr_eq(base, obj) {
                    let mut returned = false;
                    if slot.is_defined_property_accepted(ctx, desc, throwable, &mut returned)? {
                        if slot.has_offset() {
                            let old = slot.attributes();
                            slot.merge(ctx, desc);
                            if old != slot.attributes() {
                                let new_struct = obj.structure.change_attributes_transition(
                                    ctx,
                                    name,
                                    slot.attributes(),
                                );
                                obj.structure = new_struct;
                            }
                            *obj.direct_mut(slot.offset() as _) = slot.value();

                            slot.mark_put_result(PutResultType::Replace, slot.offset());
                        } else {
                            let mut offset = 0;
                            slot.merge(ctx, desc);
                            let new_struct = obj.structure.add_property_transition(
                                ctx,
                                name,
                                slot.attributes(),
                                &mut offset,
                            );
                            obj.structure = new_struct;
                            let s = &obj.structure;
                            let sz = s.storage_capacity();
                            letroot!(slots = stack, obj.slots);

                            slots.mut_handle().resize(ctx.heap(), sz as _);
                            obj.slots = *slots;
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
                let msg = JsString::new(ctx, "Object non extensible");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }

            return Ok(false);
        }

        let mut offset = 0;
        let stored = StoredSlot::new(ctx, desc);
        let s = obj
            .structure
            .add_property_transition(ctx, name, stored.attributes(), &mut offset);
        obj.structure = s;

        let s = &obj.structure;
        let sz = s.storage_capacity();
        letroot!(slots = stack, obj.slots);
        slots.mut_handle().resize(ctx.heap(), sz as _);
        obj.slots = *slots;
        *obj.direct_mut(offset as _) = stored.value();
        slot.mark_put_result(PutResultType::New, offset);
        slot.base = Some(obj.as_dyn());

        Ok(true)
    }

    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
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
                obj.get_own_indexed_property_slot(ctx, index, slot);
            }

            let mut returned = false;
            if !slot.is_not_found() {
                if let Some(base) = slot.base() {
                    if GcPointer::ptr_eq(base, obj)
                        && !slot.is_defined_property_accepted(
                            ctx,
                            desc,
                            throwable,
                            &mut returned,
                        )?
                    {
                        return Ok(returned);
                    }
                }
            }
        }

        obj.define_own_indexed_property_internal(ctx, index, desc, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_non_indexed_property_slot(ctx, name, slot) {
            return slot.get(ctx, JsValue::encode_object_value(obj.as_dyn()));
        }
        Ok(JsValue::encode_undefined_value())
    }
    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_indexed_property_slot(ctx, index, slot) {
            return slot.get(ctx, JsValue::encode_object_value(obj.as_dyn()));
        }

        Ok(JsValue::encode_undefined_value())
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        if !obj.get_own_property_slot(ctx, name, &mut slot) {
            return Ok(true);
        }

        if !slot.attributes().is_configurable() {
            if throwable {
                let msg = JsString::new(ctx, "Can not delete non configurable property");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }

        let offset = if slot.has_offset() {
            slot.offset()
        } else {
            let entry = obj.structure.get(ctx, name);
            if entry.is_not_found() {
                return Ok(true);
            }
            entry.offset
        };

        let s = obj.structure.delete_property_transition(ctx, name);
        obj.structure = s;
        *obj.direct_mut(offset as _) = JsValue::encode_empty_value();
        Ok(true)
    }

    #[allow(clippy::unnecessary_unwrap)]
    pub fn delete_indexed_internal(
        &mut self,
        _ctx: GcPointer<Context>,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if self.indexed.length() <= index {
            return Ok(true);
        }

        if self.indexed.dense() {
            if index < self.indexed.vector.size() as u32 {
                *self.indexed.vector.at_mut(index) = JsValue::encode_empty_value();
                return Ok(true);
            }

            if index < MAX_VECTOR_SIZE as u32 {
                return Ok(true);
            }
        }

        if self.indexed.map.is_none() {
            return Ok(true);
        }
        let map = self.indexed.map.as_mut().unwrap();

        match map.entry(index) {
            Entry::Vacant(_) => Ok(true),
            Entry::Occupied(x) => {
                if !x.get().attributes().is_configurable() {
                    if throwable {
                        let msg = JsString::new(_ctx, "trying to delete non-configurable property");
                        return Err(JsValue::encode_object_value(JsTypeError::new(
                            _ctx, msg, None,
                        )));
                    }
                    return Ok(false);
                }
                x.remove();
                if map.is_empty() {
                    self.indexed.make_dense();
                }
                Ok(true)
            }
        }
    }
    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if obj.class.method_table.GetOwnIndexedPropertySlot as usize
            == Self::GetOwnIndexedPropertySlotMethod as usize
        {
            return obj.delete_indexed_internal(ctx, index, throwable);
        }
        let mut slot = Slot::new();
        if !(obj.class.method_table.GetOwnIndexedPropertySlot)(obj, ctx, index, &mut slot) {
            return Ok(true);
        }

        if !slot.attributes().is_configurable() {
            if throwable {
                let msg = JsString::new(ctx, "Can not delete non configurable property");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }

        obj.delete_indexed_internal(ctx, index, throwable)
    }
    #[allow(unused_variables)]
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        obj.get_own_property_names(ctx, collector, mode);
        let mut obj = unsafe { obj.prototype_mut() };
        while let Some(proto) = obj {
            proto.get_own_property_names(ctx, collector, mode);
            obj = unsafe { proto.prototype_mut() };
        }
    }
    #[allow(unused_variables)]
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        if obj.indexed.dense() {
            for index in 0..obj.indexed.vector.size() {
                let it = obj.indexed.vector.at(index);
                if !it.is_empty() {
                    collector(Symbol::Index(index as _), u32::MAX);
                }
            }
        }

        if let Some(map) = &obj.indexed.map {
            for it in map.iter() {
                if mode == EnumerationMode::IncludeNotEnumerable
                    || it.1.attributes().is_enumerable()
                {
                    collector(Symbol::Index(*it.0), u32::MAX);
                }
            }
        }

        obj.structure.get_own_property_names(
            ctx,
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
        obj: &mut GcPointer<Self>,
        ctx: GcPointer<Context>,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        let stack = ctx.shadowstack();
        letroot!(
            args = stack,
            Arguments::new(JsValue::encode_object_value(*obj), &mut [])
        );

        macro_rules! try_ {
            ($sym: expr) => {
                let try_get = ctx.description($sym);

                let m = obj.get(ctx, $sym)?;

                if m.is_callable() {
                    let res = m
                        .get_object()
                        .downcast::<JsObject>()
                        .unwrap()
                        .as_function_mut()
                        .call(ctx, &mut args, m)?;
                    if res.is_primitive() || (res.is_undefined() || res.is_null()) {
                        return Ok(res);
                    }
                }
            };
        }

        if hint == JsHint::String {
            try_!("toString".intern());
            try_!("valueOf".intern());
        } else {
            try_!("valueOf".intern());
            try_!("toString".intern());
        }

        let msg = JsString::new(ctx, "invalid default value");
        Err(JsValue::encode_object_value(JsTypeError::new(
            ctx, msg, None,
        )))
    }
    /*const fn get_method_table() -> MethodTable {
        js_method_table!(JsObject)
    }*/

    /// create new empty JS object instance.
    pub fn new_empty(ctx: GcPointer<Context>) -> GcPointer<Self> {
        let stack = ctx.shadowstack();
        letroot!(
            structure = stack,
            ctx.global_data().empty_object_struct.unwrap()
        );
        Self::new(ctx, &structure, Self::class(), ObjectTag::Ordinary)
    }
    /// Create new JS object instance with provided class, structure and tag.
    pub fn new(
        mut ctx: GcPointer<Context>,
        structure: &GcPointer<Structure>,
        class: &'static Class,
        tag: ObjectTag,
    ) -> GcPointer<Self> {
        let stack = ctx.shadowstack();
        let init = IndexedElements::new(ctx);
        //root!(indexed = stack, ctx.heap().allocate(init));
        letroot!(
            storage = stack,
            ArrayStorage::with_size(
                ctx,
                structure.storage_capacity() as _,
                structure.storage_capacity() as _,
            )
        );
        let this = Self {
            structure: *structure,
            class,

            slots: *storage,
            object_data_start: 0,
            indexed: init,
            flags: OBJ_FLAG_EXTENSIBLE,
            tag,
        };
        ctx.heap().allocate(this)
    }

    // only for internal use
    // copy constructor and prototype
    pub fn copy(mut ctx: GcPointer<Context>, source: &mut GcPointer<JsObject>) -> GcPointer<Self> {
        let stack = ctx.shadowstack();
        let init = IndexedElements::new(ctx);
        let structure = source.structure;
        let class = source.class;
        let tag = source.tag;
        letroot!(
            storage = stack,
            ArrayStorage::with_size(
                ctx,
                source.structure.storage_capacity() as _,
                source.structure.storage_capacity() as _,
            )
        );
        let this = Self {
            structure,
            class,

            slots: *storage,
            object_data_start: 0,
            indexed: init,
            flags: OBJ_FLAG_EXTENSIBLE,
            tag,
        };
        let mut target = ctx.heap().allocate(this);
        let mut names = vec![];
        source.get_own_property_names(
            ctx,
            &mut |name, _| {
                names.push(name);
            },
            EnumerationMode::IncludeNotEnumerable,
        );
        for name in names {
            let desc = source.get_own_property(ctx, name).unwrap();
            target
                .define_own_property(ctx, name, &desc, false)
                .unwrap_or_else(|_| unreachable!());
        }
        target
    }

    pub fn tag(&self) -> ObjectTag {
        self.tag
    }
}

impl GcPointer<JsObject> {
    pub fn get_own_property_names(
        &mut self,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        (self.class.method_table.GetOwnPropertyNames)(self, ctx, collector, mode)
    }
    pub fn get_property_names(
        &mut self,
        ctx: GcPointer<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        (self.class.method_table.GetPropertyNames)(self, ctx, collector, mode)
    }
    pub fn put_non_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        (self.class.method_table.PutNonIndexedSlot)(self, ctx, name, val, slot, throwable)
    }
    #[allow(clippy::wrong_self_convention)]
    /// 7.1 Type Conversion
    ///
    /// 7.1.1 ToPrimitive
    pub fn to_primitive(
        &mut self,
        ctx: GcPointer<Context>,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        let stack = ctx.shadowstack();
        let exotic_to_prim = self.get_method(ctx, "toPrimitive".intern());

        letroot!(obj = stack, *self);
        match exotic_to_prim {
            Ok(val) => {
                // downcast_unchecked here is safe because `get_method` returns `Err` if property is not a function.
                letroot!(func = stack, unsafe {
                    val.get_object().downcast_unchecked::<JsObject>()
                });
                let f = func.as_function_mut();
                let mut tmp = [JsValue::encode_undefined_value()];
                letroot!(
                    args = stack,
                    Arguments::new(JsValue::encode_object_value(*obj), &mut tmp,)
                );

                *args.at_mut(0) = match hint {
                    JsHint::Number | JsHint::None => {
                        JsValue::encode_object_value(JsString::new(ctx, "number"))
                    }
                    JsHint::String => JsValue::encode_object_value(JsString::new(ctx, "string")),
                };

                f.call(ctx, &mut args, val)
            }
            _ => (self.class.method_table.DefaultValue)(&mut obj, ctx, hint),
        }
    }
    pub fn delete_non_indexed(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DeleteNonIndexed)(self, ctx, name, throwable)
    }
    pub fn delete(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        match name {
            Symbol::Index(index) => self.delete_indexed(ctx, index, throwable),
            name => self.delete_non_indexed(ctx, name, throwable),
        }
    }
    pub fn delete_indexed(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DeleteIndexed)(self, ctx, index, throwable)
    }

    pub fn has_indexed_property(&self) -> bool {
        let mut obj = *self;
        loop {
            if obj.structure.is_indexed() {
                return true;
            }
            match obj.prototype() {
                Some(proto) => obj = *proto,
                None => break false,
            }
        }
    }
    pub fn get_non_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetNonIndexedPropertySlot)(self, ctx, name, slot)
    }
    pub fn get_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetIndexedPropertySlot)(self, ctx, index, slot)
    }

    pub fn get_own_non_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetOwnNonIndexedPropertySlot)(self, ctx, name, slot)
    }
    pub fn can_put(&mut self, ctx: GcPointer<Context>, name: Symbol, slot: &mut Slot) -> bool {
        if let Symbol::Index(index) = name {
            self.can_put_indexed(ctx, index, slot)
        } else {
            self.can_put_non_indexed(ctx, name, slot)
        }
    }

    pub fn can_put_indexed(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        if self.get_indexed_property_slot(ctx, index, slot) {
            if slot.attributes().is_accessor() {
                return slot.accessor().setter().is_pointer()
                    && !slot.accessor().setter().is_empty();
            } else {
                return slot.attributes().is_writable();
            }
        }
        self.is_extensible()
    }
    pub fn put_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        (self.class.method_table.PutIndexedSlot)(self, ctx, index, val, slot, throwable)
    }
    pub fn put_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if let Symbol::Index(index) = name {
            self.put_indexed_slot(ctx, index, val, slot, throwable)
        } else {
            self.put_non_indexed_slot(ctx, name, val, slot, throwable)
        }
    }
    pub fn structure(&self) -> GcPointer<Structure> {
        self.structure
    }
    pub fn get_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if let Symbol::Index(index) = name {
            self.get_indexed_property_slot(ctx, index, slot)
        } else {
            self.get_non_indexed_property_slot(ctx, name, slot)
        }
    }

    pub fn get_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> PropertyDescriptor {
        let mut slot = Slot::new();
        if self.get_property_slot(ctx, name, &mut slot) {
            return slot.to_descriptor();
        }
        PropertyDescriptor::new_val(JsValue::encode_empty_value(), AttrSafe::not_found())
    }
    pub fn get_method(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
    ) -> Result<JsValue, JsValue> {
        let val = self.get(ctx, name);
        match val {
            Err(e) => Err(e),
            Ok(val) => {
                if val.is_callable() {
                    return Ok(val);
                } else {
                    let desc = ctx.description(name);
                    let msg = JsString::new(ctx, format!("Property '{}' is not a method", desc));

                    Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )))
                }
            }
        }
    }
    pub fn put(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        throwable: bool,
    ) -> Result<(), JsValue> {
        let mut slot = Slot::new();

        self.put_slot(ctx, name, val, &mut slot, throwable)
    }

    pub fn can_put_non_indexed(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if self.get_non_indexed_property_slot(ctx, name, slot) {
            if slot.attributes().is_accessor() {
                return slot.accessor().setter().is_pointer()
                    && !slot.accessor().setter().is_empty();
            } else {
                return slot.attributes().is_writable();
            }
        }
        self.is_extensible()
    }

    pub fn has_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> bool {
        let mut slot = Slot::new();
        self.get_property_slot(ctx, name, &mut slot)
    }
    pub fn has_own_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> bool {
        let mut slot = Slot::new();
        self.get_own_property_slot(ctx, name, &mut slot)
    }
    pub fn define_own_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DefineOwnIndexedPropertySlot)(
            self, ctx, index, desc, slot, throwable,
        )
    }
    pub fn get_own_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if let Symbol::Index(index) = name {
            self.get_own_indexed_property_slot(ctx, index, slot)
        } else {
            self.get_own_non_indexed_property_slot(ctx, name, slot)
        }
    }
    pub fn get(&mut self, ctx: GcPointer<Context>, name: Symbol) -> Result<JsValue, JsValue> {
        let mut slot = Slot::new();
        self.get_slot(ctx, name, &mut slot)
    }
    pub fn get_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if let Symbol::Index(index) = name {
            self.get_indexed_slot(ctx, index, slot)
        } else {
            self.get_non_indexed_slot(ctx, name, slot)
        }
    }

    pub fn get_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        (self.class.method_table.GetIndexedSlot)(self, ctx, index, slot)
    }

    pub fn get_non_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        (self.class.method_table.GetNonIndexedSlot)(self, ctx, name, slot)
    }
    pub fn get_own_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        (self.class.method_table.GetOwnIndexedPropertySlot)(self, ctx, index, slot)
        //unsafe { JsObject::GetOwnIndexedPropertySlotMethod(*self, ctx, index, slot) }
    }
    fn define_own_indexe_value_dense_internal(
        &mut self,
        mut ctx: GcPointer<Context>,
        index: u32,
        val: JsValue,
        absent: bool,
    ) {
        if index < self.indexed.vector.size() {
            if !absent {
                self.indexed.non_gc &= !val.is_object();
                *self.indexed.vector.at_mut(index) = val;
            } else {
                *self.indexed.vector.at_mut(index) = JsValue::encode_undefined_value();
            }
        } else {
            if !self.structure.is_indexed() {
                let s = self.structure.change_indexed_transition(ctx);

                self.structure = s;
            }
            let stack = ctx.shadowstack();
            letroot!(vector = stack, self.indexed.vector);
            vector.mut_handle().resize(ctx.heap(), index + 1);
            self.indexed.vector = *vector;
        }
        if !absent {
            self.indexed.non_gc &= !val.is_object();
            *self.indexed.vector.at_mut(index) = val;
        } else {
            *self.indexed.vector.at_mut(index) = JsValue::encode_undefined_value();
        }
        if index >= self.indexed.length() {
            self.indexed.set_length(index + 1);
        }
    }
    pub fn define_own_indexed_property_internal(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if index >= self.indexed.length() && !self.indexed.writable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "adding an element to the array which length is not writable is rejected",
                );
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    ctx, msg, None,
                )));
            }
            return Ok(false);
        }

        if self.indexed.dense() {
            if desc.is_default() {
                if index < MAX_VECTOR_SIZE as u32 {
                    self.define_own_indexe_value_dense_internal(
                        ctx,
                        index,
                        desc.value(),
                        desc.is_value_absent(),
                    );
                    return Ok(true);
                }
            } else {
                if is_absent_descriptor(desc)
                    && index < self.indexed.vector.size()
                    && !self.indexed.vector.at(index).is_empty()
                {
                    if !desc.is_value_absent() {
                        self.indexed.non_gc &= !desc.value().is_object();
                        *self.indexed.vector.at_mut(index) = desc.value();
                    }
                    return Ok(true);
                }

                if index < MAX_VECTOR_SIZE as u32 {
                    self.indexed.make_sparse(ctx);
                }
            }
        }

        let mut sparse = self.indexed.ensure_map(ctx);
        match sparse.get_mut(&index) {
            Some(entry) => {
                let mut returned = false;
                if entry.is_defined_property_accepted(ctx, desc, throwable, &mut returned)? {
                    self.indexed.non_gc &= !desc.value().is_object();
                    entry.merge(ctx, desc);
                }
                Ok(returned)
            }
            None if !self.is_extensible() => {
                if throwable {
                    let msg = JsString::new(ctx, "object not extensible");
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        ctx, msg, None,
                    )));
                }
                Ok(false)
            }
            None => {
                if !self.structure.is_indexed() {
                    let s = self.structure.change_indexed_transition(ctx);
                    self.structure = s;
                }
                if index >= self.indexed.length() {
                    self.indexed.set_length(index + 1);
                }
                self.indexed.non_gc &= !desc.value().is_object();
                sparse.insert(index, StoredSlot::new(ctx, desc));
                Ok(true)
            }
        }
    }

    pub fn define_own_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if let Symbol::Index(index) = name {
            self.define_own_indexed_property_slot(ctx, index, desc, slot, throwable)
        } else {
            self.define_own_non_indexed_property_slot(ctx, name, desc, slot, throwable)
        }
    }
    pub fn define_own_non_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DefineOwnNonIndexedPropertySlot)(
            self, ctx, name, desc, slot, throwable,
        )
    }
    pub fn define_own_property(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        self.define_own_property_slot(ctx, name, desc, &mut slot, throwable)
    }

    pub fn get_own_property(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
    ) -> Option<PropertyDescriptor> {
        let mut slot = Slot::new();
        if self.get_own_property_slot(ctx, name, &mut slot) {
            return Some(slot.to_descriptor());
        }
        None
    }
    #[inline]
    pub fn change_extensible(&mut self, ctx: GcPointer<Context>, val: bool) {
        if val {
            self.flags |= OBJ_FLAG_EXTENSIBLE;
        } else {
            self.flags &= !OBJ_FLAG_EXTENSIBLE;
        }

        self.structure = self.structure.change_extensible_transition(ctx);
        self.indexed.make_sparse(ctx);
    }
    pub fn freeze(&mut self, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
        let mut names = vec![];
        self.get_own_property_names(
            ctx,
            &mut |name, _| names.push(name),
            EnumerationMode::IncludeNotEnumerable,
        );

        for name in names {
            let mut desc = self.get_own_property(ctx, name).unwrap();
            if desc.is_data() {
                desc.set_writable(false);
            }
            if desc.is_configurable() {
                desc.set_configurable(false);
            }
            self.define_own_property(ctx, name, &desc, true)?;
        }
        self.change_extensible(ctx, false);

        Ok(true)
    }

    pub fn seal(&mut self, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
        let mut names = vec![];
        self.get_own_property_names(
            ctx,
            &mut |name, _| names.push(name),
            EnumerationMode::IncludeNotEnumerable,
        );

        for name in names {
            let mut desc = self.get_own_property(ctx, name).unwrap();

            if desc.is_configurable() {
                desc.set_configurable(false);
            }
            self.define_own_property(ctx, name, &desc, true)?;
        }
        self.change_extensible(ctx, false);

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        vm::{object::*, value::*, *},
        Platform,
    };

    #[test]
    fn test_put() {
        Platform::initialize();
        let options = Options::default();
        let mut rt = Runtime::new(options, None);
        let mut ctx = Context::new(&mut rt);
        let stack = rt.shadowstack();

        letroot!(object = stack, JsObject::new_empty(ctx));

        let result = object.put(ctx, "key".intern(), JsValue::new(42.4242), false);
        assert!(result.is_ok());
        rt.heap().gc();
        match object.get(ctx, "key".intern()) {
            Ok(val) => {
                assert!(val.is_number());
                assert_eq!(val.get_number(), 42.4242);
            }
            Err(_) => {
                unreachable!();
            }
        }
    }

    #[test]
    fn test_indexed() {
        Platform::initialize();
        let options = Options::default();
        let mut rt = Runtime::new(options, None);
        let mut ctx = Context::new(&mut rt);
        let stack = rt.shadowstack();

        letroot!(object = stack, JsObject::new_empty(ctx));
        for i in 0..10000u32 {
            let result = object.put(ctx, Symbol::Index(i), JsValue::new(i), false);
            assert!(result.is_ok());
        }

        rt.heap().gc();
        for i in 0..10000u32 {
            let result = object.get(ctx, Symbol::Index(i));
            match result {
                Ok(val) => {
                    assert!(val.is_number());
                    assert_eq!(val.get_number() as u32, i);
                }
                Err(_) => {
                    unreachable!();
                }
            }
        }

        let result = object.put(
            ctx,
            Symbol::Index((1024 << 6) + 1),
            JsValue::new(42.42),
            false,
        );
        assert!(result.is_ok());
        rt.heap().gc();
        for i in 0..10000u32 {
            let result = object.get(ctx, Symbol::Index(i));
            match result {
                Ok(val) => {
                    assert!(val.is_number());
                    assert_eq!(val.get_number() as u32, i);
                }
                Err(_) => {
                    unreachable!();
                }
            }
        }
        let result = object.get(ctx, Symbol::Index((1024 << 6) + 1));
        match result {
            Ok(val) => {
                assert!(val.is_number());
                assert_eq!(val.get_number(), 42.42);
            }
            Err(_) => {
                unreachable!();
            }
        }
    }
}

impl JsClass for () {
    fn class() -> &'static Class {
        JsObject::class()
    }
}

pub struct TypedJsObject<T: JsClass> {
    object: GcPointer<JsObject>,
    marker: PhantomData<T>,
}
impl<T: JsClass> JsTryFrom<GcPointer<JsObject>> for TypedJsObject<T> {
    #[inline]
    fn try_from(ctx: GcPointer<Context>, value: GcPointer<JsObject>) -> Result<Self, JsValue> {
        if likely(value.is_class(T::class())) {
            return Ok(Self {
                object: value,
                marker: PhantomData,
            });
        } else {
            Err(JsValue::new(ctx.new_type_error(format!(
                "Expected class '{}' but found '{}'",
                T::class().name,
                value.class.name
            ))))
        }
    }
}

impl<T: JsClass> JsTryFrom<JsValue> for TypedJsObject<T> {
    #[inline]
    fn try_from(ctx: GcPointer<Context>, value: JsValue) -> Result<Self, JsValue> {
        if unlikely(!value.is_jsobject()) {
            return Err(JsValue::new(ctx.new_type_error("Expected object")));
        }
        Self::try_from(ctx, value.get_jsobject())
    }
}
impl<T: JsClass> TypedJsObject<T> {
    pub fn new(value: impl Into<Self>) -> Self {
        value.into()
    }
    pub fn object(&self) -> GcPointer<JsObject> {
        self.object
    }

    pub fn seal(&mut self, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
        self.object.seal(ctx)
    }
    pub fn freeze(&mut self, ctx: GcPointer<Context>) -> Result<bool, JsValue> {
        self.object.freeze(ctx)
    }

    pub fn get_own_property(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
    ) -> Option<PropertyDescriptor> {
        self.object.get_own_property(ctx, name)
    }
    pub fn define_own_property(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        self.object.define_own_property(ctx, name, desc, throwable)
    }

    pub fn define_own_non_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        self.object
            .define_own_non_indexed_property_slot(ctx, name, desc, slot, throwable)
    }

    pub fn define_own_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        self.object
            .define_own_property_slot(ctx, name, desc, slot, throwable)
    }

    pub fn get_own_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        self.object.get_own_indexed_property_slot(ctx, index, slot)
    }

    pub fn get_non_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        self.object.get_non_indexed_slot(ctx, name, slot)
    }

    pub fn get_indexed_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        self.object.get_indexed_slot(ctx, index, slot)
    }

    pub fn get_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        self.object.get_slot(ctx, name, slot)
    }

    pub fn get(&mut self, ctx: GcPointer<Context>, name: Symbol) -> Result<JsValue, JsValue> {
        self.object.get(ctx, name)
    }

    pub fn get_own_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        self.object.get_own_property_slot(ctx, name, slot)
    }

    pub fn define_own_indexed_property_slot(
        &mut self,
        ctx: GcPointer<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        self.object
            .define_own_indexed_property_slot(ctx, index, desc, slot, throwable)
    }

    pub fn has_own_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> bool {
        self.object.has_own_property(ctx, name)
    }

    pub fn has_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> bool {
        self.object.has_property(ctx, name)
    }

    pub fn put(
        &mut self,
        ctx: GcPointer<Context>,
        name: Symbol,
        val: JsValue,
        throwable: bool,
    ) -> Result<(), JsValue> {
        self.object.put(ctx, name, val, throwable)
    }
    pub fn get_property(&mut self, ctx: GcPointer<Context>, name: Symbol) -> PropertyDescriptor {
        self.object.get_property(ctx, name)
    }
}

impl<T: JsClass> From<JsValue> for TypedJsObject<T> {
    fn from(value: JsValue) -> Self {
        assert!(value.is_jsobject(), "Object expected");
        let object = value.get_jsobject();
        assert!(
            object.is_class(T::class()),
            "Expected class '{}' but found '{}'",
            T::class().name,
            object.class.name
        );
        Self {
            object,
            marker: PhantomData,
        }
    }
}

impl<T: JsClass> From<GcPointer<JsObject>> for TypedJsObject<T> {
    fn from(value: GcPointer<JsObject>) -> Self {
        let object = value;
        assert!(
            object.is_class(T::class()),
            "Expected class '{}' but found '{}'",
            T::class().name,
            object.class.name
        );
        Self {
            object,
            marker: PhantomData,
        }
    }
}
impl<T: JsClass> Deref for TypedJsObject<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &**self.object.data::<T>()
    }
}

impl<T: JsClass> DerefMut for TypedJsObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut **self.object.data::<T>()
    }
}
unsafe impl<T: JsClass> Trace for TypedJsObject<T> {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.object.trace(visitor)
    }
}
impl<T: JsClass> Serializable for TypedJsObject<T> {
    fn serialize(&self, serializer: &mut crate::gc::snapshot::serializer::SnapshotSerializer) {
        self.object.serialize(serializer);
    }
}

impl<T: JsClass> Deserializable for TypedJsObject<T> {
    unsafe fn deserialize_inplace(
        deser: &mut crate::gc::snapshot::deserializer::Deserializer,
    ) -> Self {
        Self {
            object: unsafe { transmute(deser.get_reference()) },
            marker: PhantomData,
        }
    }
    unsafe fn deserialize(
        _at: *mut u8,
        _deser: &mut crate::gc::snapshot::deserializer::Deserializer,
    ) {
        unreachable!()
    }

    unsafe fn allocate(
        _ctx: &mut Runtime,
        _deser: &mut crate::gc::snapshot::deserializer::Deserializer,
    ) -> *mut crate::gc::cell::GcPointerBase {
        unreachable!()
    }
}

impl<T: JsClass> Clone for TypedJsObject<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: JsClass> Copy for TypedJsObject<T> {}
