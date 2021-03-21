use super::object::*;
use super::slot::*;
use super::string::*;
use super::structure::Structure;
use super::symbol_table::Symbol;
use super::value::*;
use super::Runtime;
use super::{arguments::*, code_block::CodeBlock};
use super::{array_storage::ArrayStorage, property_descriptor::*};
use super::{attributes::*, symbol_table::Internable};
use super::{error::JsTypeError, method_table::*};
use crate::heap::cell::{GcPointer, Trace, Tracer};
use std::mem::ManuallyDrop;

pub struct JsFunction {
    pub construct_struct: Option<GcPointer<Structure>>,
    pub ty: FuncType,
}

pub enum FuncType {
    Native(JsNativeFunction),
    User(JsVMFunction),
    Bound(JsBoundFunction),
}

#[allow(non_snake_case)]
impl JsFunction {
    pub fn is_native(&self) -> bool {
        matches!(self.ty, FuncType::Native(_))
    }
    pub fn is_vm(&self) -> bool {
        matches!(self.ty, FuncType::User(_))
    }
    pub fn is_bound(&self) -> bool {
        matches!(self.ty, FuncType::Bound(_))
    }
    pub fn has_instance(
        &self,
        this: &mut GcPointer<JsObject>,
        rt: &mut Runtime,
        val: JsValue,
    ) -> Result<bool, JsValue> {
        if !val.is_jsobject() {
            return Ok(false);
        }

        let got = this.get(rt, "prototype".intern())?;
        if !got.is_jsobject() {
            let msg = JsString::new(rt, "'prototype' is not object");
            return Err(JsValue::encode_object_value(JsTypeError::new(
                rt, msg, None,
            )));
        }

        let proto = got.get_jsobject();
        let mut obj = val.get_jsobject().prototype().copied();
        while let Some(obj_) = obj {
            if GcPointer::ptr_eq(&obj_, &proto) {
                return Ok(true);
            } else {
                obj = obj_.prototype().copied();
            }
        }
        Ok(false)
    }
    pub fn is_strict(&self) -> bool {
        match self.ty {
            FuncType::Native(_) => false,
            FuncType::User(ref x) => x.code.strict,
            FuncType::Bound(ref x) => x.target.as_function().is_strict(),
        }
    }

    pub fn as_native(&self) -> &JsNativeFunction {
        match self.ty {
            FuncType::Native(ref x) => x,
            _ => unreachable!(),
        }
    }
    pub fn as_native_mut(&mut self) -> &mut JsNativeFunction {
        match self.ty {
            FuncType::Native(ref mut x) => x,
            _ => unreachable!(),
        }
    }

    pub fn as_vm(&self) -> &JsVMFunction {
        match self.ty {
            FuncType::User(ref x) => x,
            _ => unreachable!(),
        }
    }
    pub fn as_vm_mut(&mut self) -> &mut JsVMFunction {
        match self.ty {
            FuncType::User(ref mut x) => x,
            _ => unreachable!(),
        }
    }

    pub fn as_bound(&self) -> &JsBoundFunction {
        match self.ty {
            FuncType::Bound(ref x) => x,
            _ => unreachable!(),
        }
    }

    pub fn as_bound_mut(&mut self) -> &mut JsBoundFunction {
        match self.ty {
            FuncType::Bound(ref mut x) => x,
            _ => unreachable!(),
        }
    }

    pub fn construct<'a>(
        &mut self,
        vm: &mut Runtime,

        args: &mut Arguments,
        structure: Option<GcPointer<Structure>>,
    ) -> Result<JsValue, JsValue> {
        let structure = structure.unwrap_or_else(|| Structure::new_unique_indexed(vm, None, false));
        let obj = JsObject::new(vm, structure, JsObject::get_class(), ObjectTag::Ordinary);
        args.ctor_call = true;
        args.this = JsValue::encode_object_value(obj);
        self.call(vm, args)
    }

    pub fn call<'a>(&mut self, vm: &mut Runtime, args: &mut Arguments) -> Result<JsValue, JsValue> {
        match self.ty {
            FuncType::Native(ref x) => (x.func)(vm, args),
            FuncType::User(ref x) => {
                vm.perform_vm_call(x, JsValue::encode_object_value(x.scope.clone()), args)
            }
            FuncType::Bound(ref x) => {
                let stack = vm.shadowstack();
                root!(
                    args = stack,
                    Arguments {
                        values: x.args.clone(),
                        this: x.this,
                        ctor_call: args.ctor_call,
                    }
                );
                let mut target = x.target.clone();
                target.as_function_mut().call(vm, &mut args)
            }
        }
    }
    pub fn new(vm: &mut Runtime, ty: FuncType, _strict: bool) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(
            vm,
            vm.global_data().get_function_struct(),
            JsFunction::get_class(),
            ObjectTag::Function,
        );

        obj.set_callable(true);

        *obj.data::<JsFunction>() = ManuallyDrop::new(JsFunction {
            construct_struct: None,
            ty,
        });

        obj
    }
    pub fn new_with_struct(
        vm: &mut Runtime,
        structure: GcPointer<Structure>,
        ty: FuncType,
        _strict: bool,
    ) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(vm, structure, JsFunction::get_class(), ObjectTag::Function);

        obj.set_callable(true);

        *obj.data::<JsFunction>() = ManuallyDrop::new(JsFunction {
            construct_struct: None,
            ty,
        });

        obj
    }
    define_jsclass!(JsFunction, Function);
    pub fn GetPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let result = JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)?;
        if name == "caller".intern() {
            slot.make_uncacheable();
            if result.is_callable()
                && result
                    .get_object()
                    .downcast::<JsObject>()
                    .unwrap()
                    .as_function()
                    .is_strict()
            {
                let msg = JsString::new(vm, "'caller' property is not accessible in strict mode");
                return Err(JsValue::encode_object_value(JsTypeError::new(
                    vm, msg, None,
                )));
            }
        }
        Ok(result)
    }

    pub fn GetIndexedSlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let function = obj.as_function_mut();
        if name == "prototype".intern() {
            // prototype override
            function.construct_struct = None;
            slot.make_uncacheable();
        }
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: &mut GcPointer<JsObject>,
        vm: &mut Runtime,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
}
pub type JsAPI = fn(vm: &mut Runtime, arguments: &Arguments) -> Result<JsValue, JsValue>;
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct JsNativeFunction {
    pub(crate) func: JsAPI,
}

impl JsNativeFunction {
    pub fn new(vm: &mut Runtime, name: Symbol, f: JsAPI, n: u32) -> GcPointer<JsObject> {
        let vm = vm;
        let mut func = JsFunction::new(vm, FuncType::Native(JsNativeFunction { func: f }), false);
        let l = "length".intern();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::encode_f64_value(n as _), NONE),
            false,
        );
        let n = "name".intern();
        let k = vm.description(name);
        let name = JsValue::encode_object_value(JsString::new(vm, &k));
        let _ = func.define_own_property(vm, n, &*DataDescriptor::new(name, NONE), false);

        func
    }
    #[allow(clippy::many_single_char_names)]
    pub fn new_with_struct(
        vm: &mut Runtime,
        s: GcPointer<Structure>,
        name: Symbol,
        f: JsAPI,
        n: u32,
    ) -> GcPointer<JsObject> {
        let vm = vm;
        let mut func = JsFunction::new_with_struct(
            vm,
            s,
            FuncType::Native(JsNativeFunction { func: f }),
            false,
        );
        let l = "length".intern();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::encode_f64_value(n as f64), NONE),
            false,
        );
        let n = "name".intern();
        let k = vm.description(name);
        let name = JsValue::encode_object_value(JsString::new(vm, &k));
        let _ = func.define_own_property(vm, n, &*DataDescriptor::new(name, NONE), false);

        func
    }
}

unsafe impl Trace for JsFunction {
    fn trace(&mut self, tracer: &mut dyn Tracer) {
        self.construct_struct.trace(tracer);
        match self.ty {
            FuncType::User(ref mut x) => {
                x.code.trace(tracer);
                x.scope.trace(tracer);
            }
            FuncType::Bound(ref mut x) => {
                x.this.trace(tracer);
                x.args.trace(tracer);
                x.target.trace(tracer);
            }
            _ => (),
        }
    }
}

#[derive(Clone)]
pub struct JsVMFunction {
    pub code: GcPointer<CodeBlock>,
    pub scope: GcPointer<JsObject>,
}
impl JsVMFunction {
    pub fn new(
        vm: &mut Runtime,
        code: GcPointer<CodeBlock>,
        env: GcPointer<JsObject>,
    ) -> GcPointer<JsObject> {
        // let vm = vm.space().new_local_context();
        let envs = Structure::new_indexed(vm, Some(env), false);
        let scope = JsObject::new(vm, envs, JsObject::get_class(), ObjectTag::Ordinary);
        let f = JsVMFunction {
            code: code.clone(),
            scope: scope,
        };

        let mut this = JsFunction::new(vm, FuncType::User(f), false);
        let mut proto = JsObject::new_empty(vm);

        let _ = proto.define_own_property(
            vm,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(this.clone()), W | C),
            false,
        );
        let desc = vm.description(code.name);
        let s = JsString::new(vm, desc);
        let _ = this.define_own_property(
            vm,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(proto), W),
            false,
        );
        let _ = this.define_own_property(
            vm,
            "name".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(s), W | C),
            false,
        );
        this
    }
}
impl GcPointer<JsObject> {
    pub fn func_construct_map(
        &mut self,
        vm: &mut Runtime,
    ) -> Result<GcPointer<Structure>, JsValue> {
        let stack = vm.shadowstack();
        root!(obj = stack, *self);
        assert_eq!(self.tag(), ObjectTag::Function);
        let func = self.as_function_mut();

        let vm = vm;
        if let Some(s) = func.construct_struct.clone() {
            return Ok(s);
        }

        let mut slot = Slot::new();
        let proto = "prototype".intern();
        let res = JsObject::GetNonIndexedSlotMethod(&mut obj, vm, proto, &mut slot)?;
        let structure = unsafe {
            Structure::new_indexed(
                vm,
                if res.is_object() && res.get_object().is::<JsObject>() {
                    Some(res.get_object().downcast_unchecked())
                } else {
                    Some(vm.global_data().get_object_prototype())
                },
                false,
            )
        };
        if slot.is_load_cacheable()
            && slot
                .base()
                .as_ref()
                .map(|base| GcPointer::ptr_eq(&base, &obj))
                .unwrap_or(false)
            && slot.attributes().is_data()
        {
            func.construct_struct = Some(structure.clone());
        }

        Ok(structure)
    }
}
use starlight_derive::GcTrace;

#[derive(GcTrace)]
pub struct JsBoundFunction {
    pub this: JsValue,
    pub args: GcPointer<ArrayStorage>,
    pub target: GcPointer<JsObject>,
}
