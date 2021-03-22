use super::{arguments::Arguments, attributes::*, error::*, string::*, value::JsValue, *};
use crate::gc::cell::{GcCell, GcPointer, Trace};
use crate::gc::{cell::Tracer, snapshot::deserializer::Deserializable};
use std::ops::{Deref, DerefMut};
#[derive(Clone, Copy)]
pub union PropertyLayout {
    data: JsValue,
    accessors: (JsValue, JsValue), // getter,setter
}
#[repr(C)]
#[derive(Clone, Copy)]

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
                accessors: (JsValue::encode_undefined_value(), setter),
            },
        }
    }
    pub fn accessor_getter(getter: JsValue, attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(
                attrs | ACCESSOR | UNDEF_VALUE | UNDEF_SETTER | UNDEF_WRITABLE,
            )),
            value: PropertyLayout {
                accessors: (getter, JsValue::encode_undefined_value()),
            },
        }
    }

    pub fn generic(attrs: u32) -> Self {
        Self {
            attrs: AttrExternal::new(Some(
                attrs | UNDEF_VALUE | UNDEF_GETTER | UNDEF_SETTER | UNDEF_WRITABLE,
            )),
            value: PropertyLayout {
                data: JsValue::encode_empty_value(),
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

    pub fn value(&self) -> JsValue {
        unsafe { self.value.data }
    }

    pub fn getter(&self) -> JsValue {
        unsafe { self.value.accessors.0 }
    }

    pub fn setter(&self) -> JsValue {
        unsafe { self.value.accessors.1 }
    }
}
#[derive(Clone, Copy)]

pub struct StoredSlot {
    pub(crate) value: JsValue,
    pub(crate) attributes: AttrSafe,
}
unsafe impl Trace for StoredSlot {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.value.trace(visitor);
    }
}

impl GcCell for StoredSlot {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    vtable_impl!();
}
impl StoredSlot {
    pub fn value(&self) -> JsValue {
        self.value
    }
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
            value: JsValue::encode_empty_value(),
            attributes: object_data(),
        }
    }

    pub fn new_raw(value: JsValue, attributes: AttrSafe) -> Self {
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
    #[allow(unused_variables)]
    pub fn get(&self, context: &mut Runtime, this_binding: JsValue) -> Result<JsValue, JsValue> {
        if self.attributes.is_data() {
            return Ok(self.value);
        }
        assert!(self.attributes.is_accessor());

        self.accessor().invoke_getter(context, this_binding)
    }

    pub fn accessor(&self) -> GcPointer<Accessor> {
        assert!(self.attributes.is_accessor());
        unsafe { self.value.get_object().downcast_unchecked() }
    }
    /// ECMA262 section 8.12.9 `[[DefineOwnProperty]]` step 5 and after
    /// this returns `[[DefineOwnProperty]]` descriptor is accepted or not,
    /// if you see return value of `[[DefineOwnProperty]]`,
    /// see bool argument returned
    ///
    /// current is currently set PropertyDescriptor, and desc is which we try to set.
    pub fn is_defined_property_accepted(
        &self,
        vm: &mut Runtime,
        desc: &PropertyDescriptor,
        throwable: bool,
        returned: &mut bool,
    ) -> Result<bool, JsValue> {
        macro_rules! reject {
            ($str: expr) => {{
                *returned = false;
                if throwable {
                    let msg = JsString::new(vm, $str);
                    return Err(JsValue::encode_object_value(JsTypeError::new(
                        vm, msg, None,
                    )));
                }
                return Ok(false);
            }};
        }

        if desc.is_absent() {
            *returned = true;
            return Ok(false);
        }
        if self.merge_with_no_effect(desc) {
            *returned = true;
            return Ok(false);
        }
        if !self.attributes().is_configurable() {
            if desc.is_configurable() {
                reject!("changing [[Configurable]] of unconfigurable property not allowed");
            }
            if !desc.is_enumerable_absent()
                && self.attributes().is_enumerable() != desc.is_enumerable()
            {
                reject!("changing [[Enumerable]] of unconfigurable property not allowed");
            }
        }
        // step 9
        if desc.is_generic() {
        } else if self.attributes().is_data() != desc.is_data() {
            if !self.attributes().is_configurable() {
                reject!("changing descriptor type of unconfigurable property not allowed");
            }
        } else {
            if self.attributes().is_data() {
                if !self.attributes().is_configurable() {
                    if !self.attributes().is_writable() {
                        if desc.is_writable() {
                            reject!("changing [[Writable]] of unconfigurable property not allowed");
                        }

                        if !JsValue::same_value(self.value, desc.value()) {
                            reject!("changing [[Value]] of readonly property not allowed");
                        }
                    }
                }
            } else {
                if !self.attributes().is_configurable() {
                    let lhs = self.accessor();
                    let rhs = AccessorDescriptor { parent: *desc };

                    if (!rhs.is_setter_absent() && (lhs.setter() != rhs.set()))
                        || (!rhs.is_getter_absent() && (lhs.getter() != rhs.get()))
                    {
                        reject!(
                            "changing [[Set]] or [[Get]] of unconfigurable property not allowed"
                        )
                    }
                }
            }
        }
        *returned = true;
        Ok(true)
    }
    /// if desc merged to current and has no effect, return true
    pub fn merge_with_no_effect(&self, desc: &PropertyDescriptor) -> bool {
        if !desc.is_configurable_absent()
            && desc.is_configurable() != self.attributes().is_configurable()
        {
            return false;
        }

        if !desc.is_enumerable_absent() && desc.is_enumerable() != self.attributes().is_enumerable()
        {
            return false;
        }

        if desc.ty() != self.attributes().ty() {
            return false;
        }

        if desc.is_data() {
            let data = DataDescriptor { parent: *desc };
            if !data.is_writable_absent() && data.is_writable() != self.attributes().is_writable() {
                return false;
            }

            if data.is_value_absent() {
                return true;
            }
            JsValue::same_value(data.value(), self.value)
        } else if desc.is_accessor() {
            let ac = self.accessor();
            unsafe {
                desc.value.accessors.0 == ac.getter() && desc.value.accessors.1 == ac.setter()
            }
        } else {
            true
        }
    }

    pub fn merge(&mut self, context: &mut Runtime, desc: &PropertyDescriptor) {
        let mut attr = AttrExternal::new(Some(self.attributes().raw()));
        if !desc.is_configurable_absent() {
            attr.set_configurable(desc.is_configurable());
        }
        if !desc.is_enumerable_absent() {
            attr.set_enumerable(desc.is_enumerable())
        }
        if desc.is_generic() {
            self.attributes = AttrSafe::un_safe(attr);
            return;
        }

        if desc.is_data() {
            attr.set_data();
            let data = DataDescriptor { parent: *desc };

            if !data.is_value_absent() {
                self.value = data.value();
            }
            if !data.is_writable_absent() {
                attr.set_writable(data.is_writable());
            }
            self.attributes = AttrSafe::un_safe(attr);
        } else {
            attr.set_accessor();
            let accs = AccessorDescriptor { parent: *desc };

            let mut ac = if self.attributes().is_accessor() {
                self.accessor()
            } else {
                let ac = Accessor::new(
                    context,
                    JsValue::encode_undefined_value(),
                    JsValue::encode_undefined_value(),
                );
                self.value = JsValue::encode_object_value(ac.clone().as_dyn());
                ac
            };
            if accs.is_getter_absent() {
                ac.set_getter(accs.get());
            } else if accs.is_setter_absent() {
                ac.set_setter(accs.set());
            } else {
                ac.set_getter(accs.get());
                ac.set_setter(accs.set());
            }
            self.attributes = AttrSafe::un_safe(attr);
        }
    }

    pub fn new(context: &mut Runtime, desc: &PropertyDescriptor) -> Self {
        let mut this = Self {
            value: JsValue::encode_undefined_value(),
            attributes: AttrSafe::not_found(),
        };
        let mut attributes = AttrExternal::new(None);
        attributes.set_configurable(desc.is_configurable());
        attributes.set_enumerable(desc.is_enumerable());
        if desc.is_data() {
            let data = DataDescriptor { parent: *desc };
            if !data.is_value_absent() {
                this.value = data.value();
            }
            attributes.set_writable(data.is_writable());
            this.attributes = create_data(attributes);
        } else if desc.is_accessor() {
            let ac = AccessorDescriptor { parent: *desc };
            let accessor = Accessor::new(context, ac.get(), ac.set());
            this.value = JsValue::encode_object_value(accessor.as_dyn());
            this.attributes = create_accessor(attributes);
        } else {
            this.attributes = create_data(attributes);
        }
        this
    }
}

pub struct Accessor {
    pub(crate) getter: JsValue,
    pub(crate) setter: JsValue,
}

impl Accessor {
    pub fn getter(&self) -> JsValue {
        self.getter
    }

    pub fn set_getter(&mut self, val: JsValue) {
        self.getter = val;
    }

    pub fn set_setter(&mut self, val: JsValue) {
        self.setter = val;
    }

    pub fn setter(&self) -> JsValue {
        self.setter
    }
    pub fn new(vm: &mut Runtime, getter: JsValue, setter: JsValue) -> GcPointer<Self> {
        let this = Self { getter, setter };
        vm.gc().allocate(this)
    }

    pub fn invoke_getter(
        &self,
        vm: &mut Runtime,
        this_binding: JsValue,
    ) -> Result<JsValue, JsValue> {
        if self.getter().is_callable() {
            let stack = vm.shadowstack();
            crate::root!(args = stack, Arguments::new(vm, this_binding, 0));

            self.getter()
                .get_object()
                .downcast::<JsObject>()
                .unwrap()
                .as_function_mut()
                .call(vm, &mut args)
        } else {
            Ok(JsValue::encode_undefined_value())
        }
    }
}

impl GcCell for Accessor {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    vtable_impl!();
}

unsafe impl Trace for Accessor {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.setter.trace(tracer);
        self.getter.trace(tracer);
    }
}
