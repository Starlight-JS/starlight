#![feature(
    core_intrinsics,
    btree_retain,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_to_usize_cast
)]
#![allow(unused_unsafe, unused_mut)]

use heap::{cell::GcPointer, snapshot::deserializer::Deserializer};
use std::sync::atomic::AtomicBool;
use vm::{
    arguments::Arguments, object::JsObject, value::JsValue, GcParams, Runtime, RuntimeParams,
};

#[macro_use]
pub mod utils;
#[macro_use]
pub mod heap;
#[macro_use]
pub mod gc;
pub mod bytecode;
pub mod codegen;
pub mod jsrt;
pub mod vm;
pub fn val_add(x: JsValue, y: JsValue, slowpath: fn(JsValue, JsValue) -> JsValue) -> JsValue {
    if x.is_double() && y.is_double() {
        return JsValue::encode_f64_value(x.get_double() + y.get_double());
    }

    slowpath(x, y)
}

pub struct Platform;
use std::sync::atomic::Ordering;
static INIT: AtomicBool = AtomicBool::new(false);

impl Platform {
    pub fn initialize() {
        if INIT
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
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
use heap::snapshot::deserializer::Deserializable;
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

    root!(function = stack, function.expect("No function"));
    assert!(function.is_callable(), "Not a callable function");

    let global = rt.global_object();
    root!(
        args = stack,
        Arguments::new(&mut rt, JsValue::encode_object_value(global), 0)
    );
    match function.as_function_mut().call(&mut rt, &mut args) {
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
