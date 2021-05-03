/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::{cell::GcCell, snapshot::serializer::Serializable},
    prelude::*,
};
pub struct NativeIterator {
    names: Vec<Symbol>,
    at: u32,
}

impl NativeIterator {
    pub fn next(&mut self) -> Option<Symbol> {
        if self.at != self.names.len() as u32 {
            let result = self.names[self.at as usize];
            self.at += 1;
            return Some(result);
        }
        None
    }

    pub fn new(rt: &mut Runtime, obj: GcPointer<dyn GcCell>) -> GcPointer<Self> {
        let mut names = vec![];
        if let Some(mut obj) = obj.downcast::<JsObject>() {
            obj.get_property_names(
                rt,
                &mut |name, _| names.push(name),
                EnumerationMode::Default,
            );
        } else if let Some(string) = obj.downcast::<JsString>() {
            for i in 0..string.as_str().len() {
                names.push(Symbol::Index(i as _));
            }
        } else {
            todo!()
        }
        rt.heap().allocate(Self { names, at: 0 })
    }
}

impl GcCell for NativeIterator {
    fn deser_pair(&self) -> (usize, usize) {
        unreachable!()
    }
}
impl Serializable for NativeIterator {
    fn serialize(&self, _serializer: &mut SnapshotSerializer) {
        unreachable!()
    }
}

unsafe impl Trace for NativeIterator {
    fn trace(&mut self, _visitor: &mut dyn Tracer) {}
}
