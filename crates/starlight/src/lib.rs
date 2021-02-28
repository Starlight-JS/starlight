#![feature(core_intrinsics, btree_retain)]
use std::sync::atomic::AtomicBool;
use vm::{value::JsValue, Runtime};

pub mod bytecode;
pub mod heap;
pub mod jsrt;
pub mod utils;
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

    pub fn new_runtime(track_allocations: bool) -> Box<Runtime> {
        Self::initialize();
        vm::Runtime::new(track_allocations)
    }
}
