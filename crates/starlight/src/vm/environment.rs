use std::mem::size_of;

use crate::prelude::*;

pub struct Environment {
    pub parent: Option<GcPointer<Self>>,

    pub values: Vec<(JsValue, bool)>,
}

impl Environment {
    pub fn new(rt: &mut Runtime, cap: u32) -> GcPointer<Self> {
        let mut vec = Vec::with_capacity(cap as _);
        for _ in 0..cap {
            vec.push((JsValue::encode_undefined_value(), true));
        }
        rt.heap().allocate(Self {
            parent: None,

            values: vec,
        })
    }

    pub fn as_slice(&self) -> &[(JsValue, bool)] {
        &self.values
    }

    pub fn as_slice_mut(&mut self) -> &mut [(JsValue, bool)] {
        &mut self.values
    }
}

impl GcCell for Environment {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
}

unsafe impl Trace for Environment {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.parent.trace(visitor);
        for (value, _) in self.values.iter_mut() {
            value.trace(visitor);
        }
    }
}

impl Deserializable for Environment {
    unsafe fn deserialize_inplace(deser: &mut Deserializer) -> Self {
        let parent = Option::<GcPointer<Self>>::deserialize_inplace(deser);

        let values = Vec::<(JsValue, bool)>::deserialize_inplace(deser);
        Self { values, parent }
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        at.cast::<Self>().write(Self::deserialize_inplace(deser))
    }

    unsafe fn allocate(rt: &mut Runtime, _deser: &mut Deserializer) -> *mut GcPointerBase {
        rt.heap()
            .allocate_raw(vtable_of_type::<Self>() as _, size_of::<Self>())
    }
}

impl Serializable for Environment {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.parent.serialize(serializer);
        self.values.serialize(serializer);
    }
}
