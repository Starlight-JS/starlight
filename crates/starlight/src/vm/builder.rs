use std::{mem::ManuallyDrop, usize};

use crate::gc::cell::GcPointer;

use super::{
    class::JsClass,
    context::Context,
    function::JsAPI,
    object::{JsObject, ObjectTag},
    structure::Structure,
    symbol_table::Symbol,
    value::JsValue,
};

use crate::prelude::*;

use super::attributes::*;

pub struct ClassBuilder {
    pub constructor: GcPointer<JsObject>,
    pub prototype: GcPointer<JsObject>,
    pub structure: GcPointer<Structure>,
    pub context: GcPointer<Context>,
}

pub trait Builtin {
    fn init(mut _ctx: GcPointer<Context>) -> Result<(), JsValue> {
        todo!();
    }
    fn native_references() -> Vec<usize> {
        vec![]
    }
}

pub trait ClassConstructor {
    fn constructor(_ctx: GcPointer<Context>, _args: &Arguments) -> Result<Self, JsValue>
    where
        Self: Sized,
    {
        panic!("You should implement your constructor method");
    }
    fn raw_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue>;
    fn init(builder: &mut ClassBuilder) -> Result<(), JsValue>;
}

default impl<T: JsClass> ClassConstructor for T {
    fn raw_constructor(mut ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
        let name = Self::class().name.into();
        let structure = ctx.get_structure(name).unwrap();
        let object = JsObject::new(ctx, &structure, Self::class(), ObjectTag::Ordinary);
        let data = Self::constructor(ctx, args)?;
        *object.data::<Self>() = ManuallyDrop::new(data);
        Ok(object.into())
    }
}

pub struct ObjectBuilder {
    context: GcPointer<Context>,
    object: GcPointer<JsObject>
}

impl ObjectBuilder {
    pub fn new(ctx: GcPointer<Context>, object: GcPointer<JsObject>) -> ObjectBuilder{
        ObjectBuilder {
            context: ctx,
            object
        }
    }
}

impl ObjectBuilder {
    pub fn method<K: Into<Symbol>>(
        &mut self,
        name: K,
        func: JsAPI,
        arity: u32,
    ) -> Result<&mut Self, JsValue> {
        let attribute = WRITABLE | CONFIGURABLE;
        def_native_method!(
            self.context,
            self.object,
            name.into(),
            func,
            arity,
            attribute
        )?;
        Ok(self)
    }

    pub fn property<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        value: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_property!(self.context, self.object, name.into(), value, attribute)?;
        Ok(self)
    }

    pub fn accessor<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_accessor!(
            self.context,
            self.object,
            name.into(),
            getter,
            setter,
            attribute
        )?;
        Ok(self)
    }

    pub fn getter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_getter!(self.context, self.object, name.into(), getter, attribute)?;
        Ok(self)
    }

    pub fn setter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_setter!(self.context, self.object, name.into(), setter, attribute)?;
        Ok(self)
    }
}

impl ClassBuilder {
    pub fn method<K: Into<Symbol>>(
        &mut self,
        name: K,
        func: JsAPI,
        arity: u32,
    ) -> Result<&mut Self, JsValue> {
        let attribute = WRITABLE | CONFIGURABLE;
        def_native_method!(
            self.context,
            self.prototype,
            name.into(),
            func,
            arity,
            attribute
        )?;
        Ok(self)
    }

    pub fn static_method<K: Into<Symbol>>(
        &mut self,
        name: K,
        func: JsAPI,
        arity: u32,
    ) -> Result<&mut Self, JsValue> {
        let attribute = WRITABLE | CONFIGURABLE;
        def_native_method!(
            self.context,
            self.constructor,
            name.into(),
            func,
            arity,
            attribute
        )?;
        Ok(self)
    }

    pub fn property<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        value: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_property!(self.context, self.prototype, name.into(), value, attribute)?;
        Ok(self)
    }

    pub fn static_property<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        value: JsValue,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_property!(
            self.context,
            self.constructor,
            name.into(),
            value,
            attribute
        )?;
        Ok(self)
    }

    pub fn accessor<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_accessor!(
            self.context,
            self.prototype,
            name.into(),
            getter,
            setter,
            attribute
        )?;
        Ok(self)
    }

    pub fn static_accessor<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_accessor!(
            self.context,
            self.constructor,
            name.into(),
            getter,
            setter,
            attribute
        )?;
        Ok(self)
    }

    pub fn getter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_getter!(self.context, self.prototype, name.into(), getter, attribute)?;
        Ok(self)
    }

    pub fn setter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_setter!(self.context, self.prototype, name.into(), setter, attribute)?;
        Ok(self)
    }

    pub fn static_getter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        getter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_getter!(
            self.context,
            self.constructor,
            name.into(),
            getter,
            attribute
        )?;
        Ok(self)
    }

    pub fn static_setter<K: Into<Symbol>, V: Into<JsValue>>(
        &mut self,
        name: K,
        setter: V,
        attribute: Raw,
    ) -> Result<&mut Self, JsValue> {
        def_native_getter!(
            self.context,
            self.constructor,
            name.into(),
            setter,
            attribute
        )?;
        Ok(self)
    }
}
