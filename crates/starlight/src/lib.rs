/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#![feature(
    core_intrinsics,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_deref,
    stmt_expr_attributes,
    const_type_id,
    pattern
)]
#![allow(
    unused_unsafe,
    unused_mut,
    clippy::missing_safety_doc,
    clippy::field_reassign_with_default,
    clippy::needless_return,
    clippy::float_cmp,
    clippy::redundant_allocation,
    clippy::single_match,
    clippy::new_ret_no_self,
    clippy::or_fun_call,
    clippy::new_without_default,
    clippy::never_loop,
    clippy::explicit_counter_loop,
    clippy::comparison_chain,
    clippy::needless_range_loop
)]

use gc::{cell::GcPointer, snapshot::deserializer::Deserializer};
use std::sync::atomic::AtomicBool;
use vm::{arguments::Arguments, object::JsObject, value::JsValue, Runtime};
#[macro_export]
macro_rules! def_native_method {
    ($vm: expr,$obj: expr,$name: ident,$func: expr,$argc: expr) => {{
        let name = stringify!($name).intern();
        let m = $crate::vm::function::JsNativeFunction::new($vm, name, $func, $argc);
        $obj.put($vm, name, JsValue::new(m), true)
    }};
    ($vm: expr,$obj: expr,$name: ident,$func: expr,$argc: expr, $attr: expr) => {{
        let name = stringify!($name).intern();
        let m = $crate::vm::function::JsNativeFunction::new($vm, name, $func, $argc);
        $obj.define_own_property(
            $vm,
            name,
            &*DataDescriptor::new(JsValue::from(m), $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! def_native_property {
    ($vm: expr, $obj: expr, $name: ident, $prop: expr) => {{
        let name = stringify!($name).intern();
        $obj.put($vm, name, JsValue::new($prop), false)
    }};
    ($vm: expr, $obj: expr, $name: ident, $prop: expr, $attr: expr) => {{
        let name = stringify!($name).intern();
        $obj.define_own_property(
            $vm,
            name,
            &*DataDescriptor::new(JsValue::new($prop), $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! def_native_accessor {
    ($vm: expr,$obj: expr,$name: ident,$get: expr,$name_set: ident,$set: expr) => {{
        let name = stringify!($name).intern();
        let m = $crate::vm::function::JsNativeFunction::new($vm, name, $func, $argc);
        $obj.put($vm, name, JsValue::new(m), true)
    }};
}

#[macro_export]
macro_rules! as_atomic {
    ($value: expr;$t: ident) => {
        unsafe { core::mem::transmute::<_, &'_ core::sync::atomic::$t>($value as *const _) }
    };
}

#[macro_use]
pub mod utils;
#[macro_use]
pub mod gc;
pub mod bytecode;
pub mod bytecompiler;
pub mod codegen;
pub mod heap;
pub mod jsrt;
pub mod options;
//pub mod tracingjit;
mod constant;
pub mod vm;
pub struct Platform;
use std::sync::atomic::Ordering;
static INIT: AtomicBool = AtomicBool::new(false);

impl Platform {
    pub fn initialize() {
        if INIT.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed) == Ok(false) {
            vm::symbol_table::initialize_symbol_table();
        }
    }

    pub fn new_runtime(
        options: Options,
        external_references: Option<&'static [usize]>,
    ) -> Box<Runtime> {
        Self::initialize();
        vm::Runtime::new(options, external_references)
    }
}

#[no_mangle]
pub extern "C" fn platform_initialize() {
    Platform::initialize();
}
use gc::snapshot::deserializer::Deserializable;

use crate::{options::Options, vm::context::Context};
#[no_mangle]
#[doc(hidden)]
pub unsafe extern "C" fn __execute_bundle(array: *const u8, size: usize) {
    let mut function = None;

    let options = Options::default();
    let gc = gc::default_heap(&options);
    let mut rt = Deserializer::deserialize(
        false,
        std::slice::from_raw_parts(array, size),
        options,
        gc,
        None,
        |deser, _rt| {
            function = Some(GcPointer::<JsObject>::deserialize_inplace(deser));
        },
    );
    let mut ctx = Context::new(&mut rt);
    let stack = rt.shadowstack();

    letroot!(function = stack, function.expect("No function"));
    letroot!(funcc = stack, *function);
    assert!(function.is_callable(), "Not a callable function");

    let global = ctx.global_object();
    letroot!(
        args = stack,
        Arguments::new(JsValue::encode_object_value(global), &mut [])
    );
    match function
        .as_function_mut()
        .call(ctx, &mut args, JsValue::new(*funcc))
    {
        Ok(x) => {
            if x.is_number() {
                drop(rt);
                std::process::exit(x.get_number().floor() as i32);
            }
        }
        Err(e) => {
            let str = e.to_string(ctx);
            match str {
                Err(_) => panic!("Failed to get error"),
                Ok(str) => {
                    eprintln!("Uncaught exception: {}", str);
                }
            }
        }
    }
}

pub mod prelude {
    pub use super::gc::{
        cell::*, snapshot::deserializer::*, snapshot::serializer::*, snapshot::Snapshot, Heap,
        MarkingConstraint, SimpleMarkingConstraint,
    };
    pub use super::letroot;
    pub use super::options::Options;
    pub use super::vm::Runtime;
    pub use super::vm::{
        arguments::Arguments,
        array::JsArray,
        attributes::*,
        class::{Class, JsClass},
        error::*,
        function::*,
        method_table::MethodTable,
        object::{EnumerationMode, JsHint, JsObject, ObjectTag},
        property_descriptor::*,
        slot::*,
        string::*,
        structure::*,
        symbol_table::*,
        value::JsFrom,
        value::JsValue,
    };
    pub use super::Platform;
    pub use crate::constant::*;
    pub use crate::define_additional_size;
    pub use crate::js_method_table;
}

pub trait JsTryFrom<T>: Sized {
    fn try_from(ctx: GcPointer<Context>, value: T) -> Result<Self, JsValue>;
}
