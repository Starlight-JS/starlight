/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#![allow(incomplete_features)]
#![feature(
    core_intrinsics,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_deref,
    stmt_expr_attributes,
    const_type_id,
    pattern,
    specialization,
    arbitrary_self_types,
    duration_constants
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

use gc::cell::GcPointer;
use options::Options;
use std::sync::atomic::AtomicBool;
use vm::{context::Context, value::JsValue, VirtualMachineRef};
#[macro_export]
macro_rules! def_native_method {
    ($vm: expr,$obj: expr,$name: ident,$func: expr,$argc: expr) => {{
        let name = stringify!($name).intern();
        let m = $crate::vm::function::JsNativeFunction::new($vm, name, $func, $argc);
        $obj.put($vm, name, JsValue::new(m), true)
    }};
    ($vm: expr,$obj: expr,$name: expr,$func: expr,$argc: expr) => {{
        let name = $name;
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
    ($vm: expr,$obj: expr,$name: expr,$func: expr,$argc: expr, $attr: expr) => {{
        let name = $name;
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
    ($vm: expr, $obj: expr, $name: expr, $prop: expr) => {{
        $obj.put($vm, $name, JsValue::new($prop), false)
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
    ($vm: expr, $obj: expr, $name: expr, $prop: expr, $attr: expr) => {{
        $obj.define_own_property(
            $vm,
            $name,
            &*DataDescriptor::new(JsValue::new($prop), $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! def_native_accessor {
    ($vm: expr,$obj: expr,$name: ident,$getter: expr,$setter: expr, $attr:expr) => {{
        let name = stringify!($name).intern();
        $obj.define_own_property(
            $vm,
            name,
            &*AccessorDescriptor::new(JsValue::new($getter), JsValue::new($setter), $attr),
            false,
        )
    }};
    ($vm: expr,$obj: expr,$name: expr,$getter: expr,$setter: expr, $attr:expr) => {{
        $obj.define_own_property(
            $vm,
            $name,
            &*AccessorDescriptor::new(JsValue::new($getter), JsValue::new($setter), $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! def_native_getter {
    ($vm: expr,$obj: expr,$name: ident,$getter: expr, $attr:expr) => {{
        let name = stringify!($name).intern();
        $obj.define_own_property(
            $vm,
            name,
            &*AccessorDescriptor::new(JsValue::new($getter), JsValue::UNDEFINED, $attr),
            false,
        )
    }};
    ($vm: expr,$obj: expr,$name: expr,$getter: expr, $attr:expr) => {{
        $obj.define_own_property(
            $vm,
            $name,
            &*AccessorDescriptor::new(JsValue::new($getter), JsValue::UNDEFINED, $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! def_native_setter {
    ($vm: expr,$obj: expr,$name: ident,$setter: expr, $attr:expr) => {{
        let name = stringify!($name).intern();
        $obj.define_own_property(
            $vm,
            name,
            &*AccessorDescriptor::new(JsValue::UNDEFINED, JsValue::new($setter), $attr),
            false,
        )
    }};
    ($vm: expr,$obj: expr,$name: expr,$setter: expr, $attr:expr) => {{
        $obj.define_own_property(
            $vm,
            $name,
            &*AccessorDescriptor::new(JsValue::UNDEFINED, JsValue::new($setter), $attr),
            false,
        )
    }};
}

#[macro_export]
macro_rules! as_atomic {
    ($value: expr;$t: ident) => {
        unsafe { core::mem::transmute::<_, &'_ core::sync::atomic::$t>($value as *const _) }
    };
}

#[no_mangle]
#[used]
#[doc(hidden)]
pub static mut LETROOT_SINK: usize = 0;

#[doc(hidden)]
pub fn letroot_sink(ref_: *const u8) {
    unsafe {
        core::ptr::write_volatile(&mut LETROOT_SINK, ref_ as usize);
    }
}

#[macro_export]
macro_rules! letroot {
    ($var : ident = $stack : expr,$val: expr) => {
        let mut $var = $val;
        $crate::letroot_sink(&$var as *const _ as *const u8);
    };
}

#[macro_use]
pub mod utils;
#[macro_use]
pub mod gc;
pub mod bytecode;
pub mod bytecompiler;
pub mod codegen;
pub mod comet;
mod constant;
pub mod generator;
pub mod interpreter;
pub mod jsrt;
pub mod options;
pub mod vm;
pub struct Platform;
use std::sync::atomic::Ordering;
static INIT: AtomicBool = AtomicBool::new(false);

impl Platform {
    pub fn initialize() {
        if INIT.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed) == Ok(false) {
            comet::cometgc::GCPlatform::initialize();
            vm::symbol_table::initialize_symbol_table();
        }
    }

    pub fn new_runtime(
        options: Options,
        external_references: Option<Vec<usize>>,
    ) -> VirtualMachineRef {
        Self::initialize();
        vm::VirtualMachine::new(options, external_references)
    }
}

#[no_mangle]
pub extern "C" fn platform_initialize() {
    Platform::initialize();
}

pub mod prelude {
    pub use super::gc::*;

    pub use super::options::Options;
    pub use super::vm::VirtualMachine;
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
