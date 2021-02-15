use crate::{
    heap::cell::{Cell, Gc, Trace},
    vm::VirtualMachine,
};

#[repr(C)]
pub struct JsString {
    str: String,
}

impl JsString {
    pub fn is_empty(&self) -> bool {
        self.str.is_empty()
    }
    pub fn new(vm: &mut VirtualMachine, as_str: impl AsRef<str>) -> Gc<Self> {
        let str = as_str.as_ref();
        let proto = Self {
           str:str.to_string()
            // len: str.len() as _,
            //data: [],
        };
        let mut cell = vm.space().alloc(proto);

        /*unsafe {
            cell.len = str.len() as _;
            std::ptr::copy_nonoverlapping(
                str.as_bytes().as_ptr(),
                cell.data.as_mut_ptr(),
                str.len(),
            );
        }*/

        cell
    }

    pub fn as_str(&self) -> &str {
        &self.str
        /*unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                self.data.as_ptr(),
                self.len as _,
            ))
        }*/
    }

    pub fn len(&self) -> u32 {
        self.str.len() as _
    }
}

impl Cell for JsString {}
unsafe impl Trace for JsString {}

#[cfg(feature = "debug-snapshots")]
impl serde::Serialize for JsString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut x = serializer.serialize_struct("JsString", 1)?;
        x.serialize_field("data", self.as_str())?;
        x.end()
    }
}
