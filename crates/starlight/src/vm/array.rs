use super::{attributes::*, indexed_elements::MAX_VECTOR_SIZE, object::*, Runtime};
use super::{
    method_table::*,
    object::EnumerationMode,
    symbol_table::{Internable, Symbol},
};
use super::{property_descriptor::PropertyDescriptor, slot::*, value::*};
use crate::heap::cell::GcPointer;
pub struct JsArray;
#[allow(non_snake_case)]
impl JsArray {
    pub fn new(vm: &mut Runtime, n: u32) -> GcPointer<JsObject> {
        let mut arr = JsObject::new(
            vm,
            vm.global_data().array_structure.unwrap(),
            Self::get_class(),
            ObjectTag::Array,
        );
        arr.indexed.set_length(n);
        arr
    }
    define_jsclass!(JsArray, Array);
    pub fn GetPropertyNamesMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        if mode == EnumerationMode::IncludeNotEnumerable {
            collector("length".intern(), 0);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == "length".intern() {
            if throwable {
                todo!()
            }
            return Ok(false);
        }
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)
    }

    pub fn GetIndexedSlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        if name == "length".intern() {
            slot.set_1(
                JsValue::encode_f64_value(obj.indexed.length() as f64),
                if obj.indexed.writable() {
                    create_data(AttrExternal::new(Some(W)))
                } else {
                    create_data(AttrExternal::new(Some(N)))
                },
                Some(obj.as_dyn()),
            );
            return true;
        }
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == "length".intern() {
            return obj.define_length_property(vm, desc, throwable);
        }
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
}

impl GcPointer<JsObject> {
    fn change_length_writable(
        &mut self,
        vm: &mut Runtime,
        writable: bool,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if !writable {
            self.indexed.make_readonly();
        } else {
            if !self.indexed.writable() {
                if throwable {
                    todo!()
                }
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn define_length_property(
        &mut self,
        ctx: &mut Runtime,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if desc.is_configurable() {
            if throwable {
                todo!()
            }
            return Ok(false);
        }

        if desc.is_enumerable() {
            if throwable {
                todo!()
            }
            return Ok(false);
        }

        if desc.is_accessor() {
            if throwable {
                todo!()
            }

            return Ok(false);
        }

        if desc.is_value_absent() {
            if !desc.is_writable_absent() {
                return self.change_length_writable(ctx, desc.is_writable(), throwable);
            }
            return Ok(true);
        }

        let new_len_double = desc.value().to_number(ctx)?;
        let new_len = new_len_double as u32;
        if new_len as f64 != new_len_double {
            todo!()
        }

        let old_len = self.indexed.length();
        if new_len == old_len {
            if !desc.is_writable_absent() {
                return self.change_length_writable(ctx, desc.is_writable(), throwable);
            }
            return Ok(true);
        }

        if !self.indexed.writable() {
            if throwable {
                todo!()
            }
            return Ok(false);
        }
        let succ = self.set_length(ctx, new_len, throwable)?;
        if !desc.is_writable_absent() {
            return self.change_length_writable(ctx, desc.is_writable(), throwable);
        }
        Ok(succ)
    }

    fn set_length(
        &mut self,
        ctx: &mut Runtime,
        len: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut old = self.indexed.length();
        if len >= old {
            self.indexed.set_length(len);
            return Ok(true);
        }

        // dense array shrink
        if self.indexed.dense() {
            if len > MAX_VECTOR_SIZE as u32 {
                if let Some(mut map) = self.indexed.map {
                    let mut copy = vec![];
                    map.iter().for_each(|x| {
                        copy.push(*x.0);
                    });
                    copy.sort_unstable();
                    for x in copy.iter() {
                        if *x >= len {
                            map.remove(x);
                        } else {
                            break;
                        }
                    }

                    if map.is_empty() {
                        self.indexed.make_dense();
                    }
                }
            } else {
                self.indexed.make_dense();
                if self.indexed.vector.size() > len {
                    self.indexed.vector.resize(ctx, len as _);
                }
            }
            self.indexed.set_length(len);
            return Ok(true);
        }
        if (old - len) < (1 << 24) {
            while len < old {
                old -= 1;
                if !self.delete_indexed_internal(ctx, old, false)? {
                    self.indexed.set_length(old + 1);
                    if throwable {
                        todo!()
                    }
                    return Ok(false);
                }
            }
            self.indexed.set_length(len);
            return Ok(true);
        }

        let mut props = Vec::new();
        self.get_own_property_names(
            ctx,
            &mut |sym, off| {
                props.push((sym, off));
            },
            EnumerationMode::IncludeNotEnumerable,
        );

        for it in props.iter().rev() {
            let sym = it.0;
            match sym {
                Symbol::Index(index) => {
                    if !self.delete_indexed_internal(ctx, index, false)? {
                        self.indexed.set_length(index + 1);
                        if throwable {
                            todo!()
                        }
                        return Ok(false);
                    }
                }
                _ => continue,
            }
        }
        self.indexed.set_length(len);

        Ok(true)
    }
}
