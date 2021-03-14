use super::Runtime;
use crate::heap::cell::{GcCell, GcPointer, Trace};
use crate::heap::snapshot::deserializer::Deserializable;
use std::mem::size_of;

#[repr(C)]
pub struct JsString {
    pub(crate) string: String,
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.string.is_empty()
    }
    pub fn new(vm: &mut Runtime, as_str: impl AsRef<str>) -> GcPointer<Self> {
        let str = as_str.as_ref();
        let proto = Self {
            string: str.to_owned(),
        };
        let cell = vm.heap().allocate(proto);

        cell
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }

    pub fn len(&self) -> u32 {
        self.string.len() as _
    }
}

unsafe impl Trace for JsString {}
impl GcCell for JsString {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    fn compute_size(&self) -> usize {
        size_of::<Self>()
    }
    vtable_impl!();
}
