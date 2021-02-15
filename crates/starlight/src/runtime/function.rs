use std::mem::ManuallyDrop;

use super::{arguments::Arguments, error::JsTypeError, symbol::*};
use super::{attributes::*, property_descriptor::PropertyDescriptor};
use super::{method_table::*, string::JsString};
use super::{object::*, structure::Structure, value::JsValue};
use super::{property_descriptor::DataDescriptor, slot::*};
use crate::{
    bytecode::ByteCode,
    heap::cell::{Gc, Trace, Tracer},
    vm::VirtualMachine,
};
pub struct JsFunction {
    construct_struct: Option<Gc<Structure>>,
    ty: FuncType,
}

pub enum FuncType {
    Native(JsNativeFunction),
    User(JsVMFunction),
}
#[allow(non_snake_case)]
impl JsFunction {
    pub fn is_strict(&self) -> bool {
        match self.ty {
            FuncType::Native(_) => false,
            FuncType::User(ref x) => x.code.strict,
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

    pub fn construct(
        &mut self,
        vm: &mut VirtualMachine,
        args: &mut Arguments,
        structure: Option<Gc<Structure>>,
    ) -> Result<JsValue, JsValue> {
        let structure = structure.unwrap_or_else(|| Structure::new_unique_indexed(vm, None, false));
        let obj = JsObject::new(vm, structure, JsObject::get_class(), ObjectTag::Ordinary);
        args.this = JsValue::new(obj);
        let _ = self.call(vm, args)?;
        Ok(args.this)
    }

    pub fn call(
        &mut self,
        vm: &mut VirtualMachine,
        args: &mut Arguments,
    ) -> Result<JsValue, JsValue> {
        match self.ty {
            FuncType::Native(ref x) => (x.func)(vm, args),
            FuncType::User(ref x) => return vm.perform_vm_call(x, JsValue::new(x.scope), args),
        }
    }
    pub fn new(ctx: &mut VirtualMachine, ty: FuncType, _strict: bool) -> Gc<JsObject> {
        let mut obj = JsObject::new(
            ctx,
            ctx.global_data().get_function_struct(),
            JsFunction::get_class(),
            ObjectTag::Function,
        );

        obj.set_callable(true);
        unsafe {
            *obj.data::<JsFunction>() = ManuallyDrop::new(JsFunction {
                construct_struct: None,
                ty,
            });
        }

        obj
    }
    pub fn new_with_struct(
        ctx: &mut VirtualMachine,
        structure: Gc<Structure>,
        ty: FuncType,
        _strict: bool,
    ) -> Gc<JsObject> {
        let mut obj = JsObject::new(ctx, structure, JsFunction::get_class(), ObjectTag::Function);

        obj.set_callable(true);
        unsafe {
            *obj.data::<JsFunction>() = ManuallyDrop::new(JsFunction {
                construct_struct: None,
                ty,
            });
        }

        obj
    }
    define_jsclass!(JsFunction, Function);
    pub fn GetPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetPropertyNamesMethod(obj, vm, collector, mode)
    }
    pub fn DefaultValueMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        hint: JsHint,
    ) -> Result<JsValue, JsValue> {
        JsObject::DefaultValueMethod(obj, vm, hint)
    }
    pub fn DefineOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DefineOwnIndexedPropertySlotMethod(obj, vm, index, desc, slot, throwable)
    }
    pub fn GetOwnIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnIndexedPropertySlotMethod(obj, vm, index, slot)
    }
    pub fn PutIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutIndexedSlotMethod(obj, vm, index, val, slot, throwable)
    }
    pub fn PutNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        val: JsValue,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<(), JsValue> {
        JsObject::PutNonIndexedSlotMethod(obj, vm, name, val, slot, throwable)
    }
    pub fn GetOwnPropertyNamesMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        collector: &mut dyn FnMut(Symbol, u32),
        mode: EnumerationMode,
    ) {
        JsObject::GetOwnPropertyNamesMethod(obj, vm, collector, mode)
    }

    pub fn DeleteNonIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteNonIndexedMethod(obj, vm, name, throwable)
    }

    pub fn DeleteIndexedMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        JsObject::DeleteIndexedMethod(obj, vm, index, throwable)
    }

    pub fn GetNonIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        let result = JsObject::GetNonIndexedSlotMethod(obj, vm, name, slot)?;
        if name == Symbol::caller() {
            slot.make_uncacheable();
            if result.is_callable() && result.as_object().as_function().is_strict() {
                let msg = JsString::new(vm, "'caller' property is not accessible in strict mode");
                return Err(JsValue::new(JsTypeError::new(vm, msg, None)));
            }
        }
        Ok(result)
    }

    pub fn GetIndexedSlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> Result<JsValue, JsValue> {
        JsObject::GetIndexedSlotMethod(obj, vm, index, slot)
    }
    pub fn GetNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetOwnNonIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetOwnNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn GetNonIndexedPropertySlot(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetNonIndexedPropertySlotMethod(obj, vm, name, slot)
    }

    pub fn DefineOwnNonIndexedPropertySlotMethod(
        mut obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        name: Symbol,
        desc: &PropertyDescriptor,
        slot: &mut Slot,
        throwable: bool,
    ) -> Result<bool, JsValue> {
        let function = obj.as_function_mut();
        if name == Symbol::prototype() {
            // prototype override
            function.construct_struct = None;
            slot.make_uncacheable();
        }
        JsObject::DefineOwnNonIndexedPropertySlotMethod(obj, vm, name, desc, slot, throwable)
    }

    pub fn GetIndexedPropertySlotMethod(
        obj: Gc<JsObject>,
        vm: &mut VirtualMachine,
        index: u32,
        slot: &mut Slot,
    ) -> bool {
        JsObject::GetIndexedPropertySlotMethod(obj, vm, index, slot)
    }
}
pub type JsAPI = fn(vm: &mut VirtualMachine, arguments: &Arguments) -> Result<JsValue, JsValue>;
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct JsNativeFunction {
    func: JsAPI,
}

impl JsNativeFunction {
    pub fn new(ctx: &mut VirtualMachine, name: Symbol, f: JsAPI, n: u32) -> Gc<JsObject> {
        let vm = ctx;
        let mut func = JsFunction::new(vm, FuncType::Native(JsNativeFunction { func: f }), false);
        let l = Symbol::length();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::new(n as i32), NONE),
            false,
        );
        let n = Symbol::name();
        let k = vm.description(name);
        let name = JsValue::new(JsString::new(vm, &k));
        let _ = func.define_own_property(vm, n, &*DataDescriptor::new(name, NONE), false);

        func
    }
    #[allow(clippy::many_single_char_names)]
    pub fn new_with_struct(
        ctx: &mut VirtualMachine,
        s: Gc<Structure>,
        name: Symbol,
        f: JsAPI,
        n: u32,
    ) -> Gc<JsObject> {
        let vm = ctx;
        let mut func = JsFunction::new_with_struct(
            vm,
            s,
            FuncType::Native(JsNativeFunction { func: f }),
            false,
        );
        let l = Symbol::length();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::new(n as i32), NONE),
            false,
        );
        let n = Symbol::name();
        let k = vm.description(name);
        let name = JsValue::new(JsString::new(vm, &k));
        let _ = func.define_own_property(vm, n, &*DataDescriptor::new(name, NONE), false);

        func
    }
}

unsafe impl Trace for JsFunction {
    fn trace(&self, tracer: &mut dyn Tracer) {
        self.construct_struct.trace(tracer);
        match self.ty {
            FuncType::User(ref x) => x.code.trace(tracer),
            _ => (),
        }
    }
}

#[derive(Clone, Copy)]
pub struct JsVMFunction {
    pub code: Gc<ByteCode>,
    pub scope: Gc<JsObject>,
}
impl JsVMFunction {
    pub fn new(vm: &mut VirtualMachine, code: Gc<ByteCode>, env: Gc<JsObject>) -> Gc<JsObject> {
        let ctx = vm.space().new_local_context();
        let envs = ctx.new_local(Structure::new_indexed(vm, Some(env), false));
        let f = JsVMFunction {
            code,
            scope: JsObject::new(vm, *envs, JsObject::get_class(), ObjectTag::Ordinary),
        };

        let mut this = ctx.new_local(JsFunction::new(vm, FuncType::User(f), false));
        let mut proto = ctx.new_local(JsObject::new_empty(vm));

        let _ = proto.define_own_property(
            vm,
            Symbol::constructor(),
            &*DataDescriptor::new(JsValue::new(*this), W | C),
            false,
        );

        let _ = this.define_own_property(
            vm,
            Symbol::prototype(),
            &*DataDescriptor::new(JsValue::new(*proto), W),
            false,
        );

        *this
    }
}
impl JsObject {
    pub fn func_construct_map(
        &mut self,
        ctx: &mut VirtualMachine,
    ) -> Result<Gc<Structure>, JsValue> {
        let obj = // Heap::from_raw is safe here as there is no way to allocate JsObject not in the GC heap.
unsafe { Gc::from_raw(self) };
        assert_eq!(self.tag(), ObjectTag::Function);
        let func = self.as_function_mut();

        let vm = ctx;
        if let Some(s) = func.construct_struct {
            return Ok(s);
        }

        let mut slot = Slot::new();
        let proto = Symbol::prototype();
        let res = Self::GetNonIndexedSlotMethod(obj, vm, proto, &mut slot)?;
        let structure = unsafe {
            Structure::new_indexed(
                vm,
                if res.is_cell() && res.as_cell().is::<JsObject>() {
                    Some(res.as_cell().downcast_unchecked())
                } else {
                    Some(vm.global_data().get_object_prototype())
                },
                false,
            )
        };
        if slot.is_load_cacheable()
            && slot
                .base()
                .map(|base| Gc::ptr_eq(base, obj))
                .unwrap_or(false)
            && slot.attributes().is_data()
        {
            func.construct_struct = Some(structure);
        }

        Ok(structure)
    }
}
