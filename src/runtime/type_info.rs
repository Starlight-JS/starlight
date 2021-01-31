//! Type information used by GC and runtime.

use super::method_table::MethodTable;
use crate::heap::{header::Header, trace::TracerPtr, util::address::Address};
pub struct TypeInfo {
    pub heap_size: extern "C" fn(object: Address) -> usize,
    pub visit_references: Option<extern "C" fn(object: Address, tracer: TracerPtr)>,
    pub needs_destruction: bool,
    pub destructor: Option<extern "C" fn(object: Address)>,
    pub parent: Option<&'static TypeInfo>,
    pub method_table: MethodTable,
}

pub trait Type {
    fn get_type_info() -> &'static TypeInfo;
}

pub fn is<T: Type>(header: &Header) -> bool {
    let addr = Address::from_ptr(T::get_type_info());
    let mut current = Address::from_ptr(header.type_info());
    while addr != current && current.is_non_null() {
        current = match header.type_info().parent {
            Some(addr) => Address::from_ptr(addr),
            None => return false,
        }
    }
    debug_assert_eq!(current, addr);
    true
}
