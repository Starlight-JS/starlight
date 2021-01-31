use super::{
    js_cell::allocate_cell,
    method_table::MethodTable,
    ref_ptr::*,
    type_info::{Type, TypeInfo},
    vm::JSVirtualMachine,
};
use crate::heap::{header::Header, util::address};
use address::Address;
use lasso::LargeSpur;
use std::mem::size_of;
#[repr(C)]
pub struct JSSymbol {
    pub(crate) header: Header,
    pub(crate) key: lasso::LargeSpur,
}

impl JSSymbol {
    pub fn new(vm: &mut JSVirtualMachine, key: impl AsRef<str>) -> Ref<JSSymbol> {
        let key = vm.interner.get_or_intern(key.as_ref());
        let value = Self {
            header: Header::empty(),
            key,
        };
        let sym = allocate_cell(vm, size_of::<Self>(), Self::get_type_info(), value);
        sym
    }

    pub fn from_interned_key(vm: &mut JSVirtualMachine, key: LargeSpur) -> Ref<JSSymbol> {
        let value = Self {
            header: Header::empty(),
            key,
        };
        let sym = allocate_cell(vm, size_of::<Self>(), Self::get_type_info(), value);
        sym
    }

    pub fn vm(&self) -> Ref<JSVirtualMachine> {
        self.header.vm()
    }
}
impl Type for JSSymbol {
    fn get_type_info() -> &'static TypeInfo {
        static SYMBOL_INFO: TypeInfo = TypeInfo {
            visit_references: None,
            heap_size: {
                extern "C" fn size(_: Address) -> usize {
                    size_of::<JSSymbol>()
                }
                size
            },
            needs_destruction: false,
            destructor: None,
            method_table: MethodTable {},
            parent: None,
        };
        &SYMBOL_INFO
    }
}

impl AsRef<str> for JSSymbol {
    fn as_ref(&self) -> &str {
        unsafe {
            let vm: &'static JSVirtualMachine = &*self.header.fast_vm().pointer;
            vm.interner.resolve(&self.key)
        }
    }
}
