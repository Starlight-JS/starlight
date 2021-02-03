use super::{
    attributes::object_data,
    attributes::*,
    class::Class,
    context::Context,
    indexed_elements::{IndexedElements, MAX_VECTOR_SIZE},
    js_cell::{allocate_cell, JsCell},
    js_function::JsFunction,
    js_value::JsValue,
    method_table::MethodTable,
    property_descriptor::{DataDescriptor, PropertyDescriptor, StoredSlot},
    ref_ptr::AsRefPtr,
    slot::{PutResultType, Slot},
    storage::FixedStorage,
    structure::Structure,
    symbol::Symbol,
};
use super::{ref_ptr::Ref, vm::JsVirtualMachine};
use crate::{
    define_jsclass,
    gc::{handle::Handle, heap_cell::HeapObject},
    heap::trace::Tracer,
};
use std::collections::hash_map::Entry;
use std::mem::size_of;

pub type ObjectSlots = FixedStorage<JsValue>;

#[repr(C)]
pub struct JsObject {
    class: &'static Class,
    structure: Handle<Structure>,
    slots: ObjectSlots,
    elements: IndexedElements,
    flags: u32,
    // We do not use Rust enums here as we do not want to allocate more than
    // needed memory for `ObjectData` type. If object's `tag` allows for non allocating
    // additional memory (i.e object is `Ordinary`) we just don't allocate additional memory.
    tag: ObjectTag,
    data: ObjectData,
}
impl JsObject {
    pub fn is_function(&self) -> bool {
        self.tag == ObjectTag::Function
    }

    pub fn get_function(&self) -> &JsFunction {
        assert!(self.is_function());
        unsafe { &self.data.function }
    }

    pub fn get_function_mut(&mut self) -> &mut JsFunction {
        assert!(self.is_function());
        unsafe { &mut self.data.function }
    }

    pub fn direct(&self, n: usize) -> &JsValue {
        &self.slots[n]
    }

    pub fn direct_mut(&mut self, n: usize) -> &mut JsValue {
        &mut self.slots[n]
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
#[allow(non_snake_case)]
impl JsObject {
    pub fn prototype(&self) -> Option<Handle<JsObject>> {
        self.structure.prototype()
    }

    pub fn get_non_indexed_property_slot(
        &self,
        ctx: Ref<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        unsafe { Self::GetNonIndexedPropertySlotMethod(Handle::from_raw(self), ctx, name, slot) }
    }
    pub fn GetNonIndexedPropertySlotMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        loop {
            if obj.get_own_non_indexed_property_slot(ctx, name, slot) {
                break true;
            }
            match obj.prototype() {
                Some(proto) => obj = proto,
                _ => break false,
            }
        }
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: Handle<Self>,
        ctx: Ref<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let entry = obj.get_structure(ctx.vm).get(ctx.vm, name);
        if !entry.is_not_found() {
            slot.set_1(
                *obj.direct(entry.offset as _),
                entry.attrs as _,
                Some(obj.as_dyn()),
            );
            return true;
        }
        false
    }

    pub fn get_own_non_indexed_property_slot(
        &self,
        ctx: Ref<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        let entry = self.get_structure(ctx.vm).get(ctx.vm, name);
        if !entry.is_not_found() {
            slot.set_1(
                *self.direct(entry.offset as _),
                entry.attrs as _,
                Some(unsafe { Handle::<JsObject>::from_raw(self).as_dyn() }),
            );
            return true;
        }
        false
    }
    pub fn can_put(&self, ctx: Ref<Context>, name: Symbol, slot: &mut Slot) -> bool {
        if let Symbol::Indexed(index) = name {
            self.can_put_indexed(ctx, index, slot)
        } else {
            self.can_put_non_indexed(ctx, name, slot)
        }
    }

    pub fn can_put_indexed(&self, ctx: Ref<Context>, index: u32, slot: &mut Slot) -> bool {
        if self.get_indexed_property_slot(ctx, index, slot) {
            if slot.attributes().is_accessor() {
                return slot.accessor().setter().is_cell() && !slot.accessor().setter().is_empty();
            } else {
                return slot.attributes().is_writable();
            }
        }
        self.is_extensible()
    }

    pub fn can_put_non_indexed(&self, ctx: Ref<Context>, name: Symbol, slot: &mut Slot) -> bool {
        if self.get_non_indexed_property_slot(ctx, name, slot) {
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

    pub fn PutNonIndexedSlotMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if !obj.can_put(ctx, name, slot) {
            if throwable {
                todo!();
            }
            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Handle::ptr_eq(*base, obj) && slot.attributes().is_data() {
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
                todo!();
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
        obj: Handle<Self>,
        ctx: Ref<Context>,
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
    pub fn has_indexed_property(&self) -> bool {
        let mut obj = unsafe { Handle::from_raw(self) };
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
    pub fn define_own_indexed_property_slot(
        &mut self,
        ctx: Ref<Context>,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        (self.class.method_table.DefineOwnIndexedPropertySlot)(
            unsafe { Handle::from_raw(self) },
            ctx,
            index,
            desc,
            slot,
            throwable,
        )
    }
    pub fn PutIndexedSlotMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
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
            obj.define_own_indexe_value_dense_internal(ctx, index, val, false);
            return Ok(());
        }
        if !obj.can_put_indexed(ctx, index, slot) {
            if throwable {
                todo!()
            }
            return Ok(());
        }
        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Handle::ptr_eq(*base, obj) && slot.attributes().is_data() {
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
                todo!();
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
    pub fn get_own_indexed_property_slot(
        &self,
        ctx: Ref<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        unsafe { Self::GetOwnIndexedPropertySlotMethod(Handle::from_raw(self), ctx, index, slot) }
    }
    pub fn GetIndexedPropertySlotMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        loop {
            if obj.get_own_indexed_property_slot(ctx, index, slot) {
                return true;
            }
            match obj.prototype() {
                Some(proto) => obj = proto,
                None => break false,
            }
        }
    }

    pub fn get_indexed_property_slot(
        &self,
        ctx: Ref<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        unsafe { Self::GetIndexedPropertySlotMethod(Handle::from_raw(self), ctx, index, slot) }
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

    pub fn get_own_property_slot(&self, ctx: Ref<Context>, name: Symbol, slot: &mut Slot) -> bool {
        if let Symbol::Indexed(index) = name {
            self.get_own_indexed_property_slot(ctx, index, slot)
        } else {
            self.get_own_non_indexed_property_slot(ctx, name, slot)
        }
    }

    fn define_own_indexe_value_dense_internal(
        &mut self,
        ctx: Ref<Context>,
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
                let s = self.structure.change_indexed_transition(ctx.vm);

                self.set_structure(ctx.vm, s)
            }

            self.elements.vector.reserve(ctx.vm, index as usize + 1);
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
    fn define_own_indexed_property_internal(
        &mut self,
        mut ctx: Ref<Context>,
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
                        ctx,
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
                    self.elements.make_sparse(ctx.vm);
                }
            }
        }

        let mut sparse = self.elements.ensure_map(ctx.vm);
        match sparse.get_mut(&index) {
            Some(entry) => {
                entry.merge(&mut *ctx, desc);
                let mut returned = false;
                if entry.is_defined_property_accepted(desc, throwable, &mut returned)? {
                    entry.merge(&mut ctx, desc);
                }
                return Ok(returned);
            }
            None if !self.is_extensible() => {
                if throwable {
                    todo!()
                }
                return Ok(false);
            }
            None => {
                if !self.structure.is_indexed() {
                    let s = self.structure.change_indexed_transition(ctx.vm);
                    self.structure = s;
                }
                if index >= self.elements.length() {
                    self.elements.set_length(index + 1);
                }
                sparse.insert(index, StoredSlot::new(&mut ctx, desc));
                Ok(true)
            }
        }
    }

    pub fn define_own_non_indexed_property_slot(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        unsafe {
            Self::DefineOwnNonIndexedPropertySlotMethod(
                Handle::from_raw(self),
                ctx,
                name,
                desc,
                slot,
                throwable,
            )
        }
    }
    pub fn define_own_property_slot(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if let Symbol::Indexed(index) = name {
            self.define_own_indexed_property_internal(ctx, index, desc, throwable)
        } else {
            self.define_own_non_indexed_property_slot(ctx, name, desc, slot, throwable)
        }
    }
    // section 8.12.9 [[DefineOwnProperty]]
    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: Handle<Self>,
        mut ctx: Ref<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if !slot.is_used() {
            obj.get_own_property_slot(ctx, name, slot);
        }

        if !slot.is_not_found() {
            if let Some(base) = slot.base() {
                if Handle::ptr_eq(*base, obj) {
                    let mut returned = false;
                    if slot.is_defined_property_accepted(desc, throwable, &mut returned)? {
                        if slot.has_offset() {
                            let old = slot.attributes();
                            slot.merge(&mut *ctx, desc);
                            if old != slot.attributes() {
                                obj.set_structure(
                                    ctx.vm,
                                    obj.get_structure(ctx.vm).change_attributes_transition(
                                        ctx.vm,
                                        name,
                                        slot.attributes(),
                                    ),
                                );
                            }
                            *obj.direct_mut(slot.offset() as _) = slot.value();
                            slot.mark_put_result(PutResultType::Replace, slot.offset());
                        } else {
                            let mut offset = 0;
                            slot.merge(&mut *ctx, desc);
                            obj.set_structure(
                                ctx.vm,
                                obj.get_structure(ctx.vm).add_property_transition(
                                    ctx.vm,
                                    name,
                                    slot.attributes(),
                                    &mut offset,
                                ),
                            );
                            let s = obj.structure;
                            obj.slots
                                .resize(ctx.vm, s.get_slots_size(), JsValue::empty());

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
        let stored = StoredSlot::new(&mut *ctx, desc);
        let s =
            obj.structure
                .add_property_transition(ctx.vm, name, stored.attributes(), &mut offset);
        obj.set_structure(ctx.vm, s);
        let s = obj.structure;
        obj.slots
            .resize(ctx.vm, s.get_slots_size(), JsValue::empty());
        *obj.direct_mut(offset as _) = stored.value();
        slot.mark_put_result(PutResultType::New, offset);
        Ok(true)
    }

    pub fn DefineOwnIndexedPropertySlotMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
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
                    if Handle::ptr_eq(*base, obj) {
                        if !slot.is_defined_property_accepted(desc, throwable, &mut returned)? {
                            return Ok(returned);
                        }
                    }
                }
            }
        }

        obj.define_own_indexed_property_internal(ctx, index, desc, throwable)
    }

    pub fn put_non_indexed_slot(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        unsafe {
            (self.class.method_table.PutNonIndexedSlot)(
                Handle::from_raw(self),
                ctx,
                name,
                val,
                slot,
                throwable,
            )
        }
    }

    pub fn put_indexed_slot(
        &mut self,
        ctx: Ref<Context>,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        unsafe {
            (self.class.method_table.PutIndexedSlot)(
                Handle::from_raw(self),
                ctx,
                index,
                val,
                slot,
                throwable,
            )
        }
    }
    pub fn put_slot(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        if let Symbol::Indexed(index) = name {
            self.put_indexed_slot(ctx, index, val, slot, throwable)
        } else {
            self.put_non_indexed_slot(ctx, name, val, slot, throwable)
        }
    }

    pub fn get_property_slot(&self, ctx: Ref<Context>, name: Symbol, slot: &mut Slot) -> bool {
        if let Symbol::Indexed(index) = name {
            self.get_indexed_property_slot(ctx, index, slot)
        } else {
            self.get_non_indexed_property_slot(ctx, name, slot)
        }
    }

    pub fn get_property(&self, ctx: Ref<Context>, name: Symbol) -> PropertyDescriptor {
        let mut slot = Slot::new();
        if self.get_property_slot(ctx, name, &mut slot) {
            return slot.to_descriptor();
        }
        PropertyDescriptor::new_val(JsValue::empty(), AttrSafe::not_found())
    }

    pub fn put(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        val: JsValue,
        throwable: bool,
    ) -> Result<(), JsValue> {
        let mut slot = Slot::new();
        self.put_slot(ctx, name, val, &mut slot, throwable)
    }

    pub fn define_own_property(
        &mut self,
        ctx: Ref<Context>,
        name: Symbol,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        self.define_own_property_slot(ctx, name, desc, &mut slot, throwable)
    }
    pub fn GetNonIndexedSlotMethod(
        obj: Handle<Self>,
        mut ctx: Ref<Context>,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_non_indexed_property_slot(ctx, name, slot) {
            return slot.get(&mut *ctx, JsValue::new_cell(obj));
        }
        Ok(JsValue::undefined())
    }
    pub fn GetIndexedSlotMethod(
        obj: Handle<Self>,
        mut ctx: Ref<Context>,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        if obj.get_indexed_property_slot(ctx, index, slot) {
            return slot.get(&mut *ctx, JsValue::new_cell(obj));
        }
        Ok(JsValue::undefined())
    }

    pub fn DeleteNonIndexedMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut slot = Slot::new();
        if !obj.get_own_property_slot(ctx, name, &mut slot) {
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
            let entry = obj.structure.get(ctx.vm, name);
            if entry.is_not_found() {
                return Ok(true);
            }
            entry.offset
        };

        let s = obj.structure.delete_property_transition(ctx.vm, name);
        obj.structure = s;
        *obj.direct_mut(offset as _) = JsValue::empty();
        Ok(true)
    }

    fn delete_indexed_internal(
        &mut self,
        ctx: Ref<Context>,
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
            Entry::Vacant(_) => return Ok(true),
            Entry::Occupied(x) => {
                if !x.get().attributes().is_configurable() {
                    if throwable {
                        todo!();
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
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
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
                todo!();
            }
            return Ok(false);
        }

        obj.delete_indexed_internal(ctx, index, throwable)
    }
    pub fn GetPropertyNamesMethod(
        obj: Handle<Self>,
        ctx: Ref<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: JsEnumerationMode,
    ) {
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: Handle<Self>,
        ctx: Ref<Context>,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: JsEnumerationMode,
    ) {
    }

    pub fn DefaultValueMethod(
        mut obj: Handle<Self>,
        ctx: Ref<Context>,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        todo!()
    }
    /*const fn get_method_table() -> MethodTable {
        js_method_table!(JsObject)
    }*/

    define_jsclass!(JsObject, Object);

    pub fn new(
        vm: impl AsRefPtr<JsVirtualMachine>,
        structure: Handle<Structure>,
        class: &'static Class,
        tag: ObjectTag,
    ) -> Handle<Self> {
        let vm = vm.as_ref_ptr();
        let this = Self {
            structure,
            class,
            slots: FixedStorage::with_capacity(vm, structure.get_slots_size()),
            data: ObjectData { ordinary: () },
            elements: IndexedElements::new(vm),
            flags: OBJ_FLAG_EXTENSIBLE,
            tag,
        };
        allocate_cell(vm, object_size_for_tag(tag), this)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectTag {
    Ordinary,
    Array,
    Set,
    Map,
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
pub fn object_size_for_tag(tag: ObjectTag) -> usize {
    let size = size_of::<JsObject>() - size_of::<ObjectData>();
    match tag {
        ObjectTag::Function => size_of::<JsFunction>() + size,
        ObjectTag::Ordinary | ObjectTag::Array => size,

        _ => size,
    }
}
#[repr(C)]
union ObjectData {
    ordinary: (),
    function: JsFunction,
}

pub const OBJ_FLAG_TUPLE: u32 = 0x4;
pub const OBJ_FLAG_CALLABLE: u32 = 0x2;
pub const OBJ_FLAG_EXTENSIBLE: u32 = 0x1;

impl HeapObject for JsObject {
    fn visit_children(&mut self, tracer: &mut dyn Tracer) {
        self.slots.data.visit_children(tracer);
        if self.elements.dense() {
            self.elements.vector.visit_children(tracer);
        }
        self.elements.map.visit_children(tracer);
        self.structure.visit_children(tracer);
    }
    fn needs_destruction(&self) -> bool {
        false
    }
}

impl JsCell for JsObject {
    fn get_structure(&self, _vm: Ref<JsVirtualMachine>) -> Handle<Structure> {
        self.structure
    }

    fn set_structure(&mut self, _vm: Ref<JsVirtualMachine>, s: Handle<Structure>) {
        self.structure = s;
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum JsHint {
    None,
    String,
    Object,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum JsEnumerationMode {
    ExcludeNotEnumerable,
    IncludeNotEnumerable,
}

#[cfg(test)]
mod tests {
    use crate::runtime::options::Options;

    use super::*;
    use structopt::StructOpt;
    #[test]
    fn test_put() {
        let mut vm = JsVirtualMachine::create(Options::from_args());
        let ctx = vm.make_context();
        let my_struct = Structure::new_(vm, &[]);
        let mut obj = JsObject::new(
            &mut vm,
            my_struct,
            JsObject::get_class(),
            ObjectTag::Ordinary,
        );

        let _ = obj.put(ctx, vm.intern("x"), JsValue::new_int(42), false);
        let val = obj.get_property(ctx, vm.intern("x"));
        assert!(val.is_data());
        assert!(val.value().is_int32());
        assert_eq!(val.value().as_int32(), 42);
        assert!(!Handle::ptr_eq(my_struct, obj.structure));
    }
}
