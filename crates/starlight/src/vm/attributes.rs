/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use std::ops::{Deref, DerefMut};

macro_rules! d {
    ($($name : ident = $val: expr),*) => {
        $(
            pub const $name : u32 = $val;
        )*
    }
}

d! {
    NONE = 0,
    WRITABLE = 1,
    ENUMERABLE = 2,
    CONFIGURABLE = 4,
    DATA = 8,
    ACCESSOR = 16,
    EMPTY = 32,
    UNDEF_WRITABLE = 64,
    UNDEF_ENUMERABLE = 128,
    UNDEF_CONFIGURABLE = 256,
    UNDEF_VALUE = 512,
    UNDEF_GETTER = 1024,
    UNDEF_SETTER = 2048,

    // short options
    N = NONE,
    W = WRITABLE,
    E = ENUMERABLE,
    C = CONFIGURABLE
}
pub type Raw = u32;
macro_rules! c {
    ($(static const $t: ident $name : ident = $val: expr);+) => {$(
        pub const $name: $t = ($val) as $t;
    )*
    };
}

c! {
    static const Raw TYPE_MASK = DATA | ACCESSOR;
    static const Raw DATA_ATTR_MASK = DATA | WRITABLE | ENUMERABLE | CONFIGURABLE;
    static const Raw ACCESSOR_ATTR_MASK = ACCESSOR | ENUMERABLE | CONFIGURABLE;

    static const Raw DEFAULT =
      UNDEF_WRITABLE | UNDEF_ENUMERABLE | UNDEF_CONFIGURABLE |
      UNDEF_VALUE | UNDEF_GETTER | UNDEF_SETTER;
    static const Raw UNDEFS = EMPTY | DEFAULT;
    static const Raw BOTH = CONFIGURABLE | ENUMERABLE
}

pub fn is_stored(attrs: Raw) -> bool {
    if (attrs & UNDEFS) != 0 {
        return false;
    }
    if (attrs & DATA) != 0 {
        return (attrs & ACCESSOR) == 0;
    }
    if (attrs & ACCESSOR) != 0 {
        return (attrs & WRITABLE) == 0;
    }
    false
}

pub fn remove_undefs(attrs: Raw) -> Raw {
    attrs & !(UNDEFS)
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttrExternal {
    pub raw: Raw,
}

impl AttrExternal {
    pub fn new(attrs: Option<Raw>) -> Self {
        Self {
            raw: attrs.unwrap_or(NONE),
        }
    }
    pub fn ty(&self) -> Raw {
        self.raw & TYPE_MASK
    }

    pub fn is_enumerable(&self) -> bool {
        (self.raw & ENUMERABLE) != 0
    }

    pub fn is_enumerable_absent(&self) -> bool {
        (self.raw & UNDEF_ENUMERABLE) != 0
    }
    pub fn set_enumerable(&mut self, val: bool) {
        if val {
            self.raw = (self.raw & !UNDEF_ENUMERABLE) | ENUMERABLE;
        } else {
            self.raw = (self.raw & !UNDEF_ENUMERABLE) & !ENUMERABLE;
        }
    }
    pub fn is_configurable(&self) -> bool {
        (self.raw & CONFIGURABLE) != 0
    }

    pub fn is_configurable_absent(&self) -> bool {
        (self.raw & UNDEF_CONFIGURABLE) != 0
    }
    pub fn set_configurable(&mut self, val: bool) {
        if val {
            self.raw = (self.raw & !UNDEF_CONFIGURABLE) | CONFIGURABLE;
        } else {
            self.raw = (self.raw & !UNDEF_CONFIGURABLE) & !CONFIGURABLE;
        }
    }

    pub fn is_writable(&self) -> bool {
        (self.raw & WRITABLE) != 0
    }

    pub fn is_writable_absent(&self) -> bool {
        (self.raw & UNDEF_WRITABLE) != 0
    }
    pub fn set_writable(&mut self, val: bool) {
        if val {
            self.raw = (self.raw & !UNDEF_WRITABLE) | WRITABLE;
        } else {
            self.raw = (self.raw & !UNDEF_WRITABLE) & !WRITABLE;
        }
    }

    pub fn is_accessor(&self) -> bool {
        (self.raw & ACCESSOR) != 0
    }

    pub fn set_accessor(&mut self) {
        self.raw &= !(DATA | WRITABLE);
        self.raw |= ACCESSOR;
    }

    pub fn is_data(&self) -> bool {
        (self.raw & DATA) != 0
    }

    pub fn set_data(&mut self) {
        self.raw &= !ACCESSOR;
        self.raw |= DATA;
    }

    pub fn is_generic(&self) -> bool {
        (self.raw & (DATA | ACCESSOR | EMPTY)) == 0
    }

    pub fn is_empty(&self) -> bool {
        (self.raw & EMPTY) != 0
    }

    pub fn is_value_absent(&self) -> bool {
        (self.raw & UNDEF_VALUE) != 0
    }

    pub fn is_getter_absent(&self) -> bool {
        (self.raw & UNDEF_GETTER) != 0
    }

    pub fn is_setter_absent(&self) -> bool {
        (self.raw & UNDEF_SETTER) != 0
    }

    pub fn is_absent(&self) -> bool {
        self.is_configurable_absent() && self.is_enumerable_absent() && self.is_generic()
    }

    pub fn is_default(&self) -> bool {
        let def = CONFIGURABLE | ENUMERABLE | DATA | WRITABLE;
        (self.raw & def) == def
    }
    fn fill_enumerable_and_configurable(&mut self) {
        if self.is_configurable_absent() {
            self.raw &= !UNDEF_CONFIGURABLE;
        }
        if self.is_enumerable_absent() {
            self.raw &= !UNDEF_ENUMERABLE;
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttrSafe {
    pub attributes: AttrExternal,
}

impl AttrSafe {
    pub fn raw(&self) -> u32 {
        self.attributes.raw
    }
    pub fn new(attr: u32) -> Self {
        Self {
            attributes: AttrExternal::new(Some(remove_undefs(attr))),
        }
    }

    pub fn not_found() -> Self {
        Self {
            attributes: AttrExternal::new(None),
        }
    }

    pub fn is_simple_data(&self) -> bool {
        let value = DATA | WRITABLE;
        (self.raw & value) == value
    }

    pub fn un_safe(attr: AttrExternal) -> Self {
        Self::new(attr.raw)
    }

    pub fn is_not_found(&self) -> bool {
        self.raw() == NONE
    }
}

pub fn create_data(mut attrs: AttrExternal) -> AttrSafe {
    attrs.fill_enumerable_and_configurable();
    attrs.set_data();
    if attrs.is_writable_absent() {
        attrs.set_writable(false);
    }
    AttrSafe::new(attrs.raw)
}
pub fn create_accessor(mut attrs: AttrExternal) -> AttrSafe {
    attrs.fill_enumerable_and_configurable();
    attrs.set_accessor();
    AttrSafe::new(attrs.raw)
}

pub fn object_data() -> AttrSafe {
    create_data(AttrExternal::new(Some(
        WRITABLE | ENUMERABLE | CONFIGURABLE,
    )))
}

pub fn object_accessor() -> AttrSafe {
    create_accessor(AttrExternal::new(Some(ENUMERABLE | CONFIGURABLE)))
}

pub fn string_length() -> AttrSafe {
    create_data(AttrExternal::new(None))
}

pub fn string_indexed() -> AttrSafe {
    create_data(AttrExternal::new(Some(ENUMERABLE)))
}
impl Deref for AttrSafe {
    type Target = AttrExternal;
    fn deref(&self) -> &Self::Target {
        &self.attributes
    }
}

impl DerefMut for AttrSafe {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.attributes
    }
}
