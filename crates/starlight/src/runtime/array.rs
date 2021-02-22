use super::{
    attributes::*, method_table::*, object::*, property_descriptor::*, slot::*, symbol::*, value::*,
};
use super::{error::JsTypeError, indexed_elements::MAX_VECTOR_SIZE, string::JsString};
use crate::{gc::cell::*, vm::*};

pub struct JsArray;
#[allow(non_snake_case)]
impl JsArray {
    pub fn new(vm: &mut VirtualMachine, n: u32) -> Gc<JsObject> {
        let mut arr = JsObject::new(
            vm,
            vm.global_data().array_structure.unwrap(),
            Self::get_class(),
            ObjectTag::Array,
        );
        arr.elements.set_length(n);
        arr
    }
    define_jsclass!(JsArray, Array);
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
        if mode == EnumerationMode::IncludeNotEnumerable {
            collector(Symbol::length(), 0);
        }
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if name == Symbol::length() {
            if throwable {
                let msg = JsString::new(vm, "delete failed").root(vm.space());
                return Err(JsValue::new(JsTypeError::new(vm, *msg, None)));
            }
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
        if name == Symbol::length() {
            slot.set_1(
                JsValue::new(obj.elements.length() as i32),
                if obj.elements.writable() {
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
        if name == Symbol::length() {
            return obj.define_length_property(vm, desc, throwable);
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

impl Gc<JsObject> {
    fn change_length_writable(
        &mut self,
        vm: &mut VirtualMachine,
        writable: bool,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if !writable {
            self.elements.make_readonly();
        } else {
            if !self.elements.writable() {
                if throwable {
                    let msg = JsString::new(
                        vm,
                        "changing [[Writable]] of unconfigurable property not allowed",
                    );
                    return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
                }
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn define_length_property(
        &mut self,
        ctx: &mut VirtualMachine,
        desc: &PropertyDescriptor,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        if desc.is_configurable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing [[Configurable]] of unconfigurable property not allowed",
                );
                return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
            }
            return Ok(false);
        }

        if desc.is_enumerable() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing [[Enumerable]] of unconfigurable property not allowed",
                );
                return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
            }
            return Ok(false);
        }

        if desc.is_accessor() {
            if throwable {
                let msg = JsString::new(
                    ctx,
                    "changing description of unconfigurable property not allowed",
                );
                return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
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
            let msg = JsString::new(ctx, "invalid array length");
            return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
        }

        let old_len = self.elements.length();
        if new_len == old_len {
            if !desc.is_writable_absent() {
                return self.change_length_writable(ctx, desc.is_writable(), throwable);
            }
            return Ok(true);
        }

        if !self.elements.writable() {
            if throwable {
                let msg = JsString::new(ctx, "'length' not writable");
                return Err(JsValue::new(JsTypeError::new(ctx, msg, None)));
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
        ctx: &mut VirtualMachine,
        len: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let mut old = self.elements.length();
        if len >= old {
            self.elements.set_length(len);
            return Ok(true);
        }

        // dense array shrink
        if self.elements.dense() {
            if len > MAX_VECTOR_SIZE as u32 {
                if let Some(mut map) = self.elements.map {
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
                        self.elements.make_dense();
                    }
                }
            } else {
                self.elements.make_dense();
                if self.elements.vector.len() > len as usize {
                    self.elements.vector.resize(ctx, len as _, JsValue::empty());
                }
            }
            self.elements.set_length(len);
            return Ok(true);
        }
        if (old - len) < (1 << 24) {
            while len < old {
                old -= 1;
                if !self.delete_indexed_internal(ctx, old, false)? {
                    self.elements.set_length(old + 1);
                    if throwable {
                        let msg = JsString::new(ctx, "failed to shrink array").root(ctx.space());
                        return Err(JsValue::new(JsTypeError::new(ctx, *msg, None)));
                    }
                    return Ok(false);
                }
            }
            self.elements.set_length(len);
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
                Symbol::Indexed(index) => {
                    if !self.delete_indexed_internal(ctx, index, false)? {
                        self.elements.set_length(index + 1);
                        if throwable {
                            let msg =
                                JsString::new(ctx, "failed to shrink array").root(ctx.space());
                            return Err(JsValue::new(JsTypeError::new(ctx, *msg, None)));
                        }
                        return Ok(false);
                    }
                }
                _ => continue,
            }
        }
        self.elements.set_length(len);

        Ok(true)
    }
}
