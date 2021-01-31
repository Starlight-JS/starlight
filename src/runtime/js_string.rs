use crate::heap::{header::Header, util::address::Address};
use std::mem::size_of;

use super::{
    js_cell::allocate_cell,
    method_table::MethodTable,
    ref_ptr::{AsRefPtr, Ref},
    type_info::{Type, TypeInfo},
    vm::JSVirtualMachine,
};

#[repr(C)]
pub struct JSString {
    pub(crate) header: Header,
    pub(crate) length: u32,
    pub(crate) data: [u8; 0],
}

impl JSString {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.length as _) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data.as_mut_ptr(), self.length as _) }
    }
    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }

    pub fn as_str_mut(&mut self) -> &mut str {
        unsafe { std::str::from_utf8_unchecked_mut(self.as_bytes_mut()) }
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn new(vm: impl AsRefPtr<JSVirtualMachine>, value: impl AsRef<str>) -> Ref<Self> {
        let str = value.as_ref();
        let value = Self {
            header: Header::empty(),
            length: str.len() as _,
            data: [],
        };

        let mut string = allocate_cell(
            vm.as_ref_ptr(),
            str.len() + size_of::<Self>(),
            Self::get_type_info(),
            value,
        );
        string.length = str.len() as _;
        unsafe {
            std::ptr::copy_nonoverlapping(
                str.as_bytes().as_ptr(),
                string.data.as_mut_ptr(),
                str.len(),
            );
        }
        string
    }

    pub fn vm(&self) -> Ref<JSVirtualMachine> {
        self.header.vm()
    }
}

impl Type for JSString {
    fn get_type_info() -> &'static TypeInfo {
        static STR_INFO: TypeInfo = TypeInfo {
            visit_references: None,
            needs_destruction: false,
            destructor: None,
            heap_size: {
                extern "C" fn sz(this: Address) -> usize {
                    Ref::new(this.to_ptr::<JSString>()).length as usize + size_of::<JSString>()
                }
                sz
            },
            parent: None,
            method_table: MethodTable {},
        };
        &STR_INFO
    }
}
