use std::mem::{size_of, ManuallyDrop};

use crate::prelude::*;
pub struct NumberObject {
    value: f64,
}

extern "C" fn deser(obj: &mut JsObject, deser: &mut Deserializer, _: &mut Runtime) {
    *obj.data::<NumberObject>() = ManuallyDrop::new(NumberObject {
        value: f64::from_bits(deser.get_u64()),
    });
}

extern "C" fn sz() -> usize {
    size_of::<NumberObject>()
}
impl NumberObject {
    define_jsclass_with_symbol!(
        JsObject,
        Object,
        Object,
        None,
        None,
        Some(deser),
        None,
        Some(sz)
    );

    pub fn new(rt: &mut Runtime, value: f64) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(
            rt,
            &rt.global_data().number_structure.unwrap(),
            Self::get_class(),
            ObjectTag::Number,
        );
        *obj.data::<Self>() = ManuallyDrop::new(Self { value });
        obj
    }
    pub fn new_plain(
        rt: &mut Runtime,
        structure: GcPointer<Structure>,
        value: f64,
    ) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(rt, &structure, Self::get_class(), ObjectTag::Number);
        *obj.data::<Self>() = ManuallyDrop::new(Self { value });
        obj
    }

    pub fn to_ref(obj: &GcPointer<JsObject>) -> &Self {
        assert!(obj.tag() == ObjectTag::Number);
        obj.data::<Self>()
    }

    pub fn to_mut(obj: &mut GcPointer<JsObject>) -> &mut Self {
        assert!(obj.tag() == ObjectTag::Number);
        obj.data::<Self>()
    }
}
