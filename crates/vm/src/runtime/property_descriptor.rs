use crate::gc::handle::Handle;

use super::{accessor::Accessor, attributes::*};
use super::{attributes::AttrExternal, js_value::JsValue};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy)]
pub union PropertyLayout {
    data: JsValue,
    accessors: (JsValue, JsValue), // getter,setter
}
#[repr(C)]
pub struct PropertyDescriptor {
    pub attrs: AttrExternal,
    pub value: PropertyLayout,
}

impl Deref for PropertyDescriptor {
    type Target = AttrExternal;
    fn deref(&self) -> &Self::Target {
        &self.attrs
    }
}

impl DerefMut for PropertyDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.attrs
    }
}

impl PropertyDescriptor {
    pub fn get_layout(&self) -> PropertyLayout {
        self.value
    }

    pub fn data_descriptor(val: JsValue, attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(attrs | DATA | UNDEF_GETTER | UNDEF_SETTER)),
            value: PropertyLayout { data: val },
        }
    }

    pub fn accessor_descriptor(getter: JsValue, setter: JsValue, attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(attrs | ACCESSOR | UNDEF_VALUE | UNDEF_WRITABLE)),
            value: PropertyLayout {
                accessors: (getter, setter),
            },
        }
    }

    pub fn accessor_setter(setter: JsValue, attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(
                attrs | ACCESSOR | UNDEF_VALUE | UNDEF_SETTER | UNDEF_WRITABLE,
            )),
            value: PropertyLayout {
                accessors: (JsValue::undefined(), setter),
            },
        }
    }
    pub fn accessor_getter(getter: JsValue, attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(
                attrs | ACCESSOR | UNDEF_VALUE | UNDEF_SETTER | UNDEF_WRITABLE,
            )),
            value: PropertyLayout {
                accessors: (getter, JsValue::undefined()),
            },
        }
    }

    pub fn generic(attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(
                attrs | UNDEF_VALUE | UNDEF_GETTER | UNDEF_SETTER | UNDEF_WRITABLE,
            )),
            value: PropertyLayout {
                data: JsValue::undefined(),
            },
        }
    }

    pub fn new_val(val: JsValue, attrs: AttrSafe) -> Self {
        Self {
            attrs: AttrExternal::new(Some(attrs.raw())),
            value: PropertyLayout { data: val },
        }
    }

    pub fn new_getter_setter(getter: JsValue, setter: JsValue, attrs: AttrSafe) -> Self {
        Self {
            attrs: AttrExternal::new(Some(attrs.raw())),
            value: PropertyLayout {
                accessors: (getter, setter),
            },
        }
    }
}

pub struct StoredSlot {
    value: JsValue,
    attributes: AttrSafe,
}

impl StoredSlot {
    pub fn set_value(&mut self, val: JsValue) {
        self.value = val;
    }

    pub fn attributes(&self) -> AttrSafe {
        self.attributes
    }
    pub fn set_attributes(&mut self, attrs: AttrSafe) {
        self.attributes = attrs;
    }

    pub fn set(&mut self, value: JsValue, attrs: AttrSafe) {
        self.value = value;
        self.attributes = attrs;
    }

    pub fn empty() -> Self {
        Self {
            value: JsValue::undefined(),
            attributes: object_data(),
        }
    }

    pub fn new(value: JsValue, attributes: AttrSafe) -> Self {
        Self { value, attributes }
    }
}

#[repr(C)]
pub struct AccessorDescriptor {
    pub parent: PropertyDescriptor,
}

impl Deref for AccessorDescriptor {
    type Target = PropertyDescriptor;
    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for AccessorDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}

impl AccessorDescriptor {
    pub fn new(get: JsValue, set: JsValue, attrs: u32) -> Self {
        Self {
            parent: PropertyDescriptor::accessor_descriptor(get, set, attrs),
        }
    }

    pub fn get(&self) -> JsValue {
        unsafe { self.value.accessors.0 }
    }

    pub fn set(&self) -> JsValue {
        unsafe { self.value.accessors.1 }
    }

    pub fn set_get(&mut self, get: JsValue) {
        self.value.accessors.0 = get;
    }

    pub fn set_set(&mut self, set: JsValue) {
        self.value.accessors.1 = set;
    }
}

pub struct DataDescriptor {
    pub parent: PropertyDescriptor,
}
impl Deref for DataDescriptor {
    type Target = PropertyDescriptor;
    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for DataDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}

impl DataDescriptor {
    pub fn set_value(&mut self, val: JsValue) {
        self.value.data = val;
    }

    pub fn value(&self) -> JsValue {
        unsafe { self.value.data }
    }

    pub fn new(val: JsValue, attrs: u32) -> Self {
        Self {
            parent: PropertyDescriptor::data_descriptor(val, attrs),
        }
    }
}

pub struct GenericDescriptor {
    pub parent: PropertyDescriptor,
}
impl Deref for GenericDescriptor {
    type Target = PropertyDescriptor;
    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for GenericDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}

impl GenericDescriptor {
    pub fn new(attrs: u32) -> Self {
        Self {
            parent: PropertyDescriptor::generic(attrs),
        }
    }
}
impl StoredSlot {
    pub fn to_descriptor(&self) -> PropertyDescriptor {
        if self.attributes().is_data() {
            return PropertyDescriptor::data_descriptor(self.value, self.attributes().raw());
        }
        let accessor = self.accessor();
        PropertyDescriptor::accessor_descriptor(
            accessor.getter(),
            accessor.setter(),
            self.attributes.raw,
        )
    }

    pub fn get(&self, this_binding: JsValue) -> Result<JsValue, JsValue> {
        if self.attributes.is_data() {
            return Ok(self.value);
        }
        todo!("Invoke accessor")
    }

    pub fn accessor(&self) -> Handle<Accessor> {
        assert!(self.attributes.is_accessor());
        unsafe { self.value.as_cell().donwcast_unchecked() }
    }
}
