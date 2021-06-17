/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use super::structure::Structure;
use super::symbol_table::Symbol;
use super::value::*;
use super::Runtime;
use super::{arguments::*, code_block::CodeBlock};
use super::{array_storage::ArrayStorage, property_descriptor::*};
use super::{attributes::*, symbol_table::Internable};
use super::{environment::Environment, object::*};
use super::{error::JsRangeError, string::*};
use super::{error::JsTypeError, method_table::*};
use super::{interpreter::frame::CallFrame, slot::*};
use crate::gc::{
    cell::{GcPointer, Trace, Tracer},
    snapshot::{deserializer::Deserializer, serializer::SnapshotSerializer},
};
use std::{intrinsics::unlikely, mem::ManuallyDrop};

pub struct JsFunction {
    pub construct_struct: Option<GcPointer<Structure>>,
    pub ty: FuncType,
}

pub enum FuncType {
    Native(JsNativeFunction),
    Closure(JsClosureFunction),
    User(JsVMFunction),
    Bound(JsBoundFunction),
    Generator(JsGeneratorFunction),
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
    pub fn is_generator(&self) -> bool {
        matches!(self.ty, FuncType::Generator(_))
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
            FuncType::Closure(_) => false,
            FuncType::User(ref x) => x.code.strict,
            FuncType::Bound(ref x) => x.target.as_function().is_strict(),
            FuncType::Generator(ref x) => x.function.as_function().is_strict(),
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
    pub fn as_generator(&self) -> &JsGeneratorFunction {
        match self.ty {
            FuncType::Generator(ref x) => x,
            _ => unreachable!(),
        }
    }
    pub fn as_generator_mut(&mut self) -> &mut JsGeneratorFunction {
        match self.ty {
            FuncType::Generator(ref mut x) => x,
            _ => unreachable!(),
        }
    }

    pub fn construct<'a>(
        &mut self,
        vm: &mut Runtime,

        args: &mut Arguments,
        structure: Option<GcPointer<Structure>>,
        this_fn: JsValue,
    ) -> Result<JsValue, JsValue> {
        if unlikely(self.is_generator()) {
            return Err(JsValue::new(
                vm.new_type_error("function not a constructor"),
            ));
        }
        let stack = vm.shadowstack();
        letroot!(
            structure = stack,
            structure.unwrap_or_else(|| Structure::new_unique_indexed(vm, None, false))
        );
        let obj = JsObject::new(vm, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        args.ctor_call = true;
        args.this = JsValue::encode_object_value(obj);
        self.call(vm, args, this_fn)
    }

    pub fn call<'a>(
        &mut self,
        vm: &mut Runtime,
        args: &mut Arguments,
        this: JsValue,
    ) -> Result<JsValue, JsValue> {
        match self.ty {
            FuncType::Native(ref x) => (x.func)(vm, args),
            FuncType::Closure(ref x) => (x.func)(vm, args),
            FuncType::User(ref x) => {
                vm.perform_vm_call(x, JsValue::encode_object_value(x.scope.clone()), args, this)
            }
            FuncType::Bound(ref mut x) => {
                let stack = vm.shadowstack();
                letroot!(
                    args = stack,
                    Arguments {
                        this: x.this,
                        ctor_call: args.ctor_call,
                        values: x.args.as_slice_mut(),
                    }
                );
                let mut target = x.target.clone();
                target.as_function_mut().call(vm, &mut args, this)
            }
            FuncType::Generator(ref mut x) => x.call(vm, args, this),
        }
    } /*
      pub fn call_with_env<'a>(
          &mut self,
          vm: &mut Runtime,
          args: &mut Arguments,
          env: GcPointer<JsObject>,
      ) -> Result<JsValue, JsValue> {
          match self.ty {
              FuncType::Native(ref x) => (x.func)(vm, args),
              FuncType::User(ref x) => {
                  let structure = Structure::new_indexed(vm, Some(env), false);
                  let scope =
                      JsObject::new(vm, &structure, JsObject::get_class(), ObjectTag::Ordinary);
                  vm.perform_vm_call(x, JsValue::encode_object_value(x.scope.clone()), args)
              }
              FuncType::Bound(ref mut x) => {
                  let stack = vm.shadowstack();
                  root!(
                      args = stack,
                      Arguments {
                          this: x.this,
                          ctor_call: args.ctor_call,
                          values: x.args.as_slice_mut(),
                      }
                  );
                  let mut target = x.target.clone();
                  target.as_function_mut().call(vm, &mut args)
              }
          }
      }*/
    pub fn new(vm: &mut Runtime, ty: FuncType, _strict: bool) -> GcPointer<JsObject> {
        let mut obj = JsObject::new(
            vm,
            &vm.global_data().get_function_struct(),
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
        structure: &GcPointer<Structure>,
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
            &*DataDescriptor::new(JsValue::new(n as i32), NONE),
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
        s: &GcPointer<Structure>,
        name: Symbol,
        f: JsAPI,
        n: u32,
    ) -> GcPointer<JsObject> {
        let vm = vm;
        let stack = vm.shadowstack();
        letroot!(
            func = stack,
            JsFunction::new_with_struct(
                vm,
                s,
                FuncType::Native(JsNativeFunction { func: f }),
                false,
            )
        );
        let l = "length".intern();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::new(n as f64), NONE),
            false,
        );
        let n = "name".intern();
        let k = vm.description(name);
        let name = JsValue::encode_object_value(JsString::new(vm, &k));
        let _ = func.define_own_property(vm, n, &*DataDescriptor::new(name, NONE), false);

        *func
    }
}

/// Represents a javascript Function based on a rust closure
/// this is useful as an alternative for functions for example when adding a callback to an EventTarget or a Promise
/// Please note that using this will result in the code block not being serializable due to the nature of using closures
/// # Example
/// ```
/// // get the global object
/// use starlight::vm::symbol_table::Internable;
/// use starlight::vm::value::JsValue;
/// use starlight::Platform;
/// use starlight::vm::{RuntimeParams, GcParams};
///
/// // start a runtime
/// Platform::initialize();
/// let options = RuntimeParams::default();
/// let gc_params = GcParams::default();
/// let mut starlight_runtime = Platform::new_runtime(options, gc_params, None);
///
/// let mut global = starlight_runtime.global_object();
///
/// // create a symbol for the functions name
/// let name_symbol = "myFunction".intern();
/// let x = 1234;
///
/// // create a Function based on a closure
/// let arg_count = 0;
/// let func = starlight::vm::function::JsClosureFunction::new(
///     &mut starlight_runtime,
///     name_symbol,
///     move |vm, args| {
///         return Ok(JsValue::encode_int32(x));
///     },
///     arg_count,
/// );
///
/// // add the function to the global object
/// global.put(&mut starlight_runtime, name_symbol, JsValue::new(func), true);
///
/// // run the function
/// let outcome = starlight_runtime.eval("return (myFunction());").ok().expect("function failed");
/// assert_eq!(outcome.get_int32(), 1234);
/// ```
pub struct JsClosureFunction {
    pub(crate) func: Box<dyn Fn(&mut Runtime, &Arguments) -> Result<JsValue, JsValue>>,
}

impl JsClosureFunction {
    /// create a new JsClosureFunction
    pub fn new<F>(vm: &mut Runtime, name: Symbol, f: F, arg_count: u32) -> GcPointer<JsObject>
    where
        F: Fn(&mut Runtime, &Arguments) -> Result<JsValue, JsValue> + 'static,
    {
        let vm = vm;
        let mut func = JsFunction::new(
            vm,
            FuncType::Closure(JsClosureFunction { func: Box::new(f) }),
            false,
        );
        let l = "length".intern();

        let _ = func.define_own_property(
            vm,
            l,
            &*DataDescriptor::new(JsValue::new(arg_count as i32), NONE),
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
            FuncType::Generator(ref mut x) => {
                x.function.trace(tracer);
            }
            _ => (),
        }
    }
}

unsafe impl Trace for JsVMFunction {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.code.trace(visitor);
        self.scope.trace(visitor);
    }
}

#[derive(Clone)]
pub struct JsVMFunction {
    pub code: GcPointer<CodeBlock>,
    pub scope: GcPointer<Environment>,
}
impl JsVMFunction {
    pub fn new(
        vm: &mut Runtime,
        code: GcPointer<CodeBlock>,
        env: GcPointer<Environment>,
    ) -> GcPointer<JsObject> {
        // let vm = vm.space().new_local_context();
        let stack = vm.shadowstack();
        //root!(envs = stack, Structure::new_indexed(vm, Some(env), false));
        //root!(scope = stack, Environment::new(vm, 0));
        let f = JsVMFunction {
            code: code.clone(),
            scope: env,
        };
        vm.heap().defer();
        letroot!(this = stack, JsFunction::new(vm, FuncType::User(f), false));
        letroot!(proto = stack, JsObject::new_empty(vm));
        vm.heap().undefer();
        let _ = proto.define_own_property(
            vm,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(this.clone()), W | C),
            false,
        );
        let desc = vm.description(code.name);
        letroot!(s = stack, JsString::new(vm, desc));
        let _ = this.define_own_property(
            vm,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(*proto), W),
            false,
        );
        let _ = this.define_own_property(
            vm,
            "name".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(*s), W | C),
            false,
        );
        *this
    }
}
impl GcPointer<JsObject> {
    pub fn func_construct_map(
        &mut self,
        vm: &mut Runtime,
    ) -> Result<GcPointer<Structure>, JsValue> {
        let stack = vm.shadowstack();
        letroot!(obj = stack, *self);
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

/// interpreter call frame copied allocated on the heap. It is used again copied to interpreter stack
/// when function execution state is restored.
pub struct HeapCallFrame {
    pub stack: Vec<JsValue>,
    pub env: GcPointer<Environment>,
    pub code_block: GcPointer<CodeBlock>,
    pub this: JsValue,
    pub sp: usize,
    pub ip: *mut u8,
    pub try_stack: Vec<(Option<GcPointer<Environment>>, *mut u8, usize)>,
}

impl HeapCallFrame {
    /// Saves function state
    pub(crate) unsafe fn save(cf: &mut CallFrame) -> Self {
        let sp = cf.sp.offset_from(cf.limit);

        assert!(sp >= 0);
        let sp = sp as usize;
        let mut try_stack = vec![];
        for (env, ip, sp) in cf.try_stack.iter() {
            let isp = (*sp).offset_from(cf.limit) as usize;
            try_stack.push((*env, *ip, isp));
        }
        let mut stack = Vec::with_capacity(sp);
        let mut scan = cf.limit;
        let end = cf.sp;
        while scan < end {
            stack.push(scan.read());
            scan = scan.add(1);
        }
        Self {
            sp,
            try_stack,
            stack,
            code_block: cf.code_block.unwrap(),
            ip: cf.ip,
            this: cf.this,
            env: cf.env,
        }
    }

    /// Restores function state.
    pub(crate) unsafe fn restore(&mut self, cf: &mut CallFrame) {
        for val in self.stack.iter() {
            cf.push(*val);
        }
        assert_eq!(cf.limit.add(self.sp), cf.sp);
        cf.this = self.this;
        cf.ip = self.ip;
        cf.code_block = Some(self.code_block);
        cf.env = self.env;
        for (env, ip, csp) in self.try_stack.iter() {
            let csp = cf.limit.add(*csp);
            cf.try_stack.push((*env, *ip, csp));
        }
    }
}

pub struct JsGeneratorFunction {
    pub(crate) function: GcPointer<JsObject>,
}

extern "C" fn drop_generator(obj: &mut JsObject) {
    unsafe {
        ManuallyDrop::drop(obj.data::<GeneratorData>());
    }
}

extern "C" fn generator_deser(_: &mut JsObject, _: &mut Deserializer, _: &mut Runtime) {
    unreachable!("cannot deserialize generator");
}
extern "C" fn generator_ser(_: &JsObject, _: &mut SnapshotSerializer) {
    unreachable!("cannot serialize generator");
}

extern "C" fn generator_size() -> usize {
    std::mem::size_of::<GeneratorData>()
}
#[allow(improper_ctypes_definitions)]
extern "C" fn generator_trace(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<GeneratorData>().func_state.trace(tracer);
}
impl JsGeneratorFunction {
    pub fn new(vm: &mut Runtime, func: GcPointer<JsObject>) -> GcPointer<JsObject> {
        // let vm = vm.space().new_local_context();
        let stack = vm.shadowstack();
        //root!(envs = stack, Structure::new_indexed(vm, Some(env), false));
        //root!(scope = stack, Environment::new(vm, 0));
        let code = func.as_function().as_vm().code;
        let f = JsGeneratorFunction { function: func };
        vm.heap().defer();
        letroot!(
            this = stack,
            JsFunction::new(vm, FuncType::Generator(f), false)
        );
        letroot!(proto = stack, JsObject::new_empty(vm));
        vm.heap().undefer();
        let _ = proto.define_own_property(
            vm,
            "constructor".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(this.clone()), W | C),
            false,
        );
        let desc = vm.description(code.name);
        letroot!(s = stack, JsString::new(vm, desc));
        let _ = this.define_own_property(
            vm,
            "prototype".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(*proto), W),
            false,
        );
        let _ = this.define_own_property(
            vm,
            "name".intern(),
            &*DataDescriptor::new(JsValue::encode_object_value(*s), W | C),
            false,
        );
        *this
    }

    define_jsclass_with_symbol!(
        JsObject,
        Generator,
        Object,
        Some(drop_generator),
        Some(generator_trace),
        Some(generator_deser),
        Some(generator_ser),
        Some(generator_size)
    );
    /// Call generator function creating new generator object instance.
    ///
    /// ## Algorithm
    /// - Invoke function and execute it up to `OP_INITIAL_YIELD`, this opcode is inserted at start of the function
    /// when all arguments is initialized.
    /// - Pop call frame and save it onto heap allocate [HeapCallFrame].
    /// - Allocate JS object with class of [JsGeneratorFunction::get_class] and set its generator data.
    /// - Return generator object.
    fn call(
        &mut self,
        vm: &mut Runtime,
        args: &mut Arguments,
        this: JsValue,
    ) -> Result<JsValue, JsValue> {
        // execute up to OP_INITIAL_YIELD. It does return `undefined` value.
        let ret = self.function.as_function_mut().call(vm, args, this)?;
        debug_assert!(ret.is_undefined());
        let mut state = vm.stack.pop_frame().expect("Empty call stack");
        let state = unsafe { HeapCallFrame::save(&mut state) };
        let proto = vm.global_data().generator_structure.unwrap();
        let mut generator = JsObject::new(vm, &proto, Self::get_class(), ObjectTag::Ordinary);
        *generator.data::<GeneratorData>() = ManuallyDrop::new(GeneratorData {
            state: GeneratorState::Suspended,
            func_state: AsyncFunctionState {
                frame: Box::new(state),
                throw: false,
            },
        });
        Ok(JsValue::new(generator))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GeneratorMagic {
    Next,
    Return,
    Throw,
}
fn async_func_resume(vm: &mut Runtime, state: &mut AsyncFunctionState) -> Result<JsValue, JsValue> {
    let mut frame = vm
        .stack
        .new_frame(0, JsValue::encode_undefined_value(), state.frame.env)
        .ok_or_else(|| {
            let msg = JsString::new(vm, "stack overflow");
            JsValue::new(JsRangeError::new(vm, msg, None))
        })?;
    unsafe {
        state.frame.restore(&mut *frame);
        (*frame).exit_on_return = true;
        crate::vm::interpreter::eval(vm, frame)
    }
}
pub(crate) fn js_generator_next(
    vm: &mut Runtime,
    this: JsValue,
    args: &Arguments,
    magic: GeneratorMagic,
    pdone: &mut u32,
) -> Result<JsValue, JsValue> {
    let object = this.to_object(vm)?;
    if unlikely(!object.is_class(JsGeneratorFunction::get_class())) {
        return Err(JsValue::new(vm.new_type_error("not a generator")));
    }
    *pdone = 1;
    let mut ret;
    let s = object.data::<GeneratorData>();
    loop {
        match s.state {
            GeneratorState::Suspended => {
                if magic == GeneratorMagic::Next {
                    s.func_state.throw = false;
                    s.state = GeneratorState::Executing;
                    let func_ret = async_func_resume(vm, &mut s.func_state);

                    if let Err(e) = func_ret {
                        s.state = GeneratorState::Complete;
                        return Err(e);
                    }
                    let func_ret = func_ret?;
                    s.state = GeneratorState::Yield;
                    if func_ret.is_native_value() {
                        let frame = vm.stack.pop_frame();
                        let mut frame = frame.unwrap();
                        ret = frame.top();

                        unsafe {
                            *frame.at(-1) = JsValue::encode_undefined_value();
                        }
                        s.func_state.frame = Box::new(unsafe { HeapCallFrame::save(&mut frame) });
                        if func_ret.get_native_u32() == FuncRet::YieldStar as u32 {
                            s.state = GeneratorState::YieldStar;
                            *pdone = 2;
                        } else {
                            *pdone = 0;
                        }
                    } else {
                        ret = func_ret;
                        s.state = GeneratorState::Complete;
                    }
                    return Ok(ret);
                } else {
                    break;
                }
            }
            GeneratorState::Yield | GeneratorState::YieldStar => {
                ret = args.at(0);
                if magic == GeneratorMagic::Throw && s.state == GeneratorState::Yield {
                    s.func_state.throw = true;
                    return Err(ret);
                } else {
                    *s.func_state.frame.stack.last_mut().unwrap() = ret;
                }
                s.state = GeneratorState::Executing;
                let func_ret = async_func_resume(vm, &mut s.func_state).map_err(|e| {
                    s.state = GeneratorState::Complete;
                    e
                })?;
                s.state = GeneratorState::Yield;

                if func_ret.is_native_value() {
                    let frame = vm.stack.pop_frame();
                    let mut frame = frame.unwrap();

                    ret = frame.top();
                    unsafe {
                        *frame.at(-1) = JsValue::encode_undefined_value();
                    }
                    s.func_state.frame = Box::new(unsafe { HeapCallFrame::save(&mut frame) });
                    if func_ret.get_native_u32() == FuncRet::YieldStar as u32 {
                        s.state = GeneratorState::YieldStar;
                        *pdone = 2;
                    } else {
                        *pdone = 0;
                    }
                } else {
                    ret = func_ret;
                    s.state = GeneratorState::Complete;
                }
                return Ok(ret);
            }
            GeneratorState::Executing => {
                return Err(JsValue::new(
                    vm.new_type_error("cannot invoke a running generator"),
                ));
            }
            GeneratorState::Complete => break,
        }
    }

    match magic {
        GeneratorMagic::Next => {
            ret = JsValue::encode_undefined_value();
        }
        GeneratorMagic::Return => {
            ret = args.at(0);
        }
        GeneratorMagic::Throw => {
            return Err(args.at(0));
        }
    }
    Ok(ret)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GeneratorState {
    Suspended,
    Yield,
    YieldStar,
    Executing,
    Complete,
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum FuncRet {
    Await,
    YieldStar,
    Yield,
}
pub struct GeneratorData {
    pub state: GeneratorState,
    pub func_state: AsyncFunctionState,
}

pub struct AsyncFunctionData {
    pub resolving_funcs: [JsValue; 2],
    pub is_active: bool,
    pub func_state: AsyncFunctionState,
}
pub struct AsyncFunctionState {
    pub throw: bool,
    pub frame: Box<HeapCallFrame>,
}

unsafe impl Trace for AsyncFunctionState {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.frame.stack.trace(visitor);
        self.frame.env.trace(visitor);
        self.frame.code_block.trace(visitor);
        self.frame.this.trace(visitor);
        self.frame
            .try_stack
            .iter_mut()
            .for_each(|(env, _, _)| env.trace(visitor));
    }
}
