/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::value::HashValueZero;
use crate::prelude::*;
use std::collections::HashMap;
use std::intrinsics::*;
pub type MapInternal = HashMap<HashValueZero, JsValue>;

pub struct JsMap {
    storage: MapInternal,
}

impl JsMap {
    pub fn storage_mut(&mut self) -> &mut MapInternal {
        &mut self.storage
    }

    pub fn storage(&self) -> &MapInternal {
        &self.storage
    }

    pub fn has(&self, val: JsValue) -> bool {
        let val = HashValueZero(val);
        self.storage.contains_key(&val)
    }

    pub fn get(&self, val: JsValue) -> JsValue {
        let key = HashValueZero(val);
        self.storage
            .get(&key)
            .copied()
            .unwrap_or(JsValue::encode_undefined_value())
    }

    pub fn set(&mut self, key: JsValue, val: JsValue) -> Option<JsValue> {
        self.storage.insert(HashValueZero(key), val)
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }

    pub fn delete(&mut self, key: JsValue) -> Option<JsValue> {
        self.storage.remove(&HashValueZero(key))
    }

    pub fn initialize(vm: &mut Runtime, input: JsValue, it: JsValue) -> Result<JsValue, JsValue> {
        if unlikely(!input.is_jsobject()) {
            return Err(JsValue::new(
                vm.new_type_error("MapInitialize to non-object"),
            ));
        }

        let stack = vm.shadowstack();
        letroot!(obj = stack, input.get_jsobject());
        if unlikely(!obj.is_extensible()) {
            return Err(JsValue::new(
                vm.new_type_error("MapInitialize to un-extensible object"),
            ));
        }
        let mut iterable = None;
        let mut adder = None;
        if !it.is_undefined() {
            iterable = Some(it.to_object(vm)?);
            let val = obj.get(vm, "set".intern())?;
            if unlikely(!val.is_callable()) {
                return Err(JsValue::new(
                    vm.new_type_error("MapInitialize adder, `obj.set` is not callable"),
                ));
            }
            adder = Some(val.get_jsobject());
        }

        let mut data = vm.heap().allocate(MapInternal::new());
        obj.define_own_property(
            vm,
            "[[MapData]]".intern().private(),
            &*DataDescriptor::new(JsValue::new(data), W | C | E),
            false,
        )?;

        if let Some(mut iterable) = iterable {
            let mut names = vec![];
            iterable.get_own_property_names(
                vm,
                &mut |name, _| {
                    names.push(name);
                },
                EnumerationMode::Default,
            );

            for name in names {
                let v = iterable.get(vm, name)?;
                letroot!(item = stack, v.to_object(vm)?);
                let key = item.get(vm, Symbol::Index(0))?;
                let value = item.get(vm, Symbol::Index(1))?;
                let mut slice = [key, value];
                letroot!(
                    arg_list = stack,
                    Arguments::new(JsValue::encode_undefined_value(), &mut slice)
                );
                adder.unwrap().as_function_mut().call(
                    vm,
                    &mut arg_list,
                    JsValue::encode_undefined_value(),
                )?;
            }
        }
        Ok(JsValue::new(*obj))
    }
}
