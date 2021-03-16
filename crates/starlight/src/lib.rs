#![feature(
    core_intrinsics,
    btree_retain,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_to_usize_cast
)]
use std::sync::atomic::AtomicBool;
use vm::{value::JsValue, GcParams, Runtime, RuntimeParams};

#[macro_use]
pub mod utils;
#[macro_use]
pub mod heap;
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
