#![feature(
    core_intrinsics,
    btree_retain,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_to_usize_cast,
    try_trait,
    const_type_id,
    pattern
)]
#![allow(unused_unsafe, unused_mut)]

use gc::{cell::GcPointer, snapshot::deserializer::Deserializer};
use std::sync::atomic::AtomicBool;
use vm::{
    arguments::Arguments, object::JsObject, value::JsValue, GcParams, Runtime, RuntimeParams,
};
#[macro_export]
macro_rules! def_native_method {
    ($vm: expr,$obj: expr,$name: ident,$func: expr,$argc: expr) => {{
        let name = stringify!($name).intern();
        let m = $crate::vm::function::JsNativeFunction::new($vm, name, $func, $argc);
        $obj.put($vm, name, JsValue::new(m), true)
    }};
}

#[macro_use]
pub mod utils;
#[macro_use]
pub mod gc;
pub mod bytecode;
pub mod bytecompiler;
pub mod codegen;
pub mod jit;
pub mod jsrt;
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
        options: RuntimeParams,
        gc_params: GcParams,
        external_references: Option<&'static [usize]>,
    ) -> Box<Runtime> {
        Self::initialize();
        vm::Runtime::new(options, gc_params, external_references)
    }
}

#[no_mangle]
pub extern "C" fn platform_initialize() {
    Platform::initialize();
}
use gc::snapshot::deserializer::Deserializable;
#[no_mangle]
pub unsafe extern "C" fn __execute_bundle(array: *const u8, size: usize) {
    let mut function = None;
    let mut rt = Deserializer::deserialize(
        false,
        std::slice::from_raw_parts(array, size),
        RuntimeParams::default(),
        gc::default_heap(GcParams::default().with_parallel_marking(true)),
        None,
        |deser, _rt| {
            function = Some(GcPointer::<JsObject>::deserialize_inplace(deser));
        },
    );
    let stack = rt.shadowstack();

    letroot!(function = stack, function.expect("No function"));
    letroot!(funcc = stack, *&*function);
    assert!(function.is_callable(), "Not a callable function");

    let global = rt.global_object();
    letroot!(
        args = stack,
        Arguments::new(JsValue::encode_object_value(global), &mut [])
    );
    match function
        .as_function_mut()
        .call(&mut rt, &mut args, JsValue::new(*funcc))
    {
        Ok(x) => {
            if x.is_number() {
                drop(rt);
                std::process::exit(x.get_number().floor() as i32);
            }
        }
        Err(e) => {
            let str = e.to_string(&mut rt);
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
    pub use super::vm::{
        arguments::Arguments,
        array::JsArray,
        attributes::*,
        class::Class,
        error::*,
        function::*,
        method_table::MethodTable,
        object::{EnumerationMode, JsHint, JsObject, ObjectTag},
        property_descriptor::*,
        slot::*,
        string::*,
        structure::*,
        symbol_table::*,
        value::JsValue,
    };
    pub use super::vm::{GcParams, Runtime, RuntimeParams};
    pub use super::Platform;
}
