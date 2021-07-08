// TODO: Use mimalloc there?
use crate::prelude::*;
use std::{
    intrinsics::unlikely,
    mem::{size_of, ManuallyDrop},
    ptr::null_mut,
};

use super::class::JsClass;
pub struct JsArrayBuffer {
    pub(crate) data: *mut u8,
    pub(crate) size: usize,
    pub(crate) attached: bool,
}

extern "C" fn drop_array_buffer(x: &mut JsObject) {
    unsafe {
        x.data::<JsArrayBuffer>().detach();
        ManuallyDrop::drop(x.data::<JsArrayBuffer>());
    }
}

extern "C" fn array_buffer_serialize(x: &JsObject, serializer: &mut SnapshotSerializer) {
    let data = x.data::<JsArrayBuffer>();
    data.attached.serialize(serializer);
    (data.size as u32).serialize(serializer);
    if data.attached {
        assert!(!data.data.is_null());
        for i in 0..data.size {
            unsafe {
                data.data.add(i).read().serialize(serializer);
            }
        }
    }
}
extern "C" fn array_buffer_deserialize(
    x: &mut JsObject,
    deser: &mut Deserializer,
    _rt: &mut Runtime,
) {
    unsafe {
        let attached = bool::deserialize_inplace(deser);
        let size = u32::deserialize_inplace(deser) as usize;
        let mut buf = null_mut();
        if attached {
            buf = libc::malloc(size).cast::<u8>();
            for i in 0..size {
                buf.add(i).write(u8::deserialize_inplace(deser));
            }
        }
        *x.data::<JsArrayBuffer>() = ManuallyDrop::new(JsArrayBuffer {
            attached,
            data: buf,
            size,
        })
    }
}
extern "C" fn array_buffer_size() -> usize {
    size_of::<JsArrayBuffer>()
}
impl JsArrayBuffer {
    define_jsclass_with_symbol!(
        JsObject,
        ArrayBuffer,
        ArrayBuffer,
        Some(drop_array_buffer),
        None,
        Some(array_buffer_deserialize),
        Some(array_buffer_serialize),
        Some(array_buffer_size)
    );
    pub fn get_data_block(&self) -> *mut u8 {
        self.data
    }

    pub fn new(rt: &mut Runtime) -> GcPointer<JsObject> {
        let structure = rt.global_data().array_buffer_structure.unwrap();
        let mut this = JsObject::new(rt, &structure, Self::get_class(), ObjectTag::ArrayBuffer);
        *this.data::<Self>() = ManuallyDrop::new(Self {
            data: null_mut(),
            attached: false,
            size: 0,
        });
        this
    }
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn attached(&self) -> bool {
        self.attached
    }

    pub fn data(&self) -> &[u8] {
        assert!(!self.data.is_null());
        unsafe { std::slice::from_raw_parts(self.data, self.size()) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        assert!(!self.data.is_null());
        unsafe { std::slice::from_raw_parts_mut(self.data, self.size()) }
    }

    pub fn detach(&mut self) {
        if !self.data.is_null() {
            unsafe {
                libc::free(self.data.cast());
                self.data = null_mut();
                self.size = 0;
            }
        }
        self.attached = false;
    }

    pub fn create_data_block(
        &mut self,
        rt: &mut Runtime,
        size: usize,
        zero: bool,
    ) -> Result<(), JsValue> {
        self.detach();
        if size == 0 {
            self.attached = true;
            return Ok(());
        }

        if unlikely(size > u32::MAX as usize) {
            let msg = JsString::new(rt, "Cannot allocate a data block for the ArrayBuffer");
            return Err(JsValue::new(JsRangeError::new(rt, msg, None)));
        }
        unsafe {
            self.data = if zero {
                libc::calloc(1, size).cast()
            } else {
                libc::malloc(size).cast()
            };

            if unlikely(self.data.is_null()) {
                let msg = JsString::new(rt, "Cannot allocate a data block for the ArrayBuffer");
                return Err(JsValue::new(JsRangeError::new(rt, msg, None)));
            }
            self.attached = true;
            self.size = size;
        }
        Ok(())
    }
}

impl JsClass for JsArrayBuffer {
    fn class() -> &'static Class {
        Self::get_class()
    }
}
