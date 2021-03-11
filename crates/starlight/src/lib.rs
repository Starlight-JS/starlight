#![feature(
    core_intrinsics,
    btree_retain,
    llvm_asm,
    linked_list_cursors,
    destructuring_assignment,
    const_raw_ptr_to_usize_cast
)]
use std::sync::atomic::AtomicBool;
use vm::{value::JsValue, Runtime};
#[macro_use]
pub mod utils;
pub mod bytecode;
pub mod heap;
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
        track_allocations: bool,
        external_references: Option<Box<[usize]>>,
    ) -> Box<Runtime> {
        Self::initialize();
        vm::Runtime::new(track_allocations, external_references)
    }
}
