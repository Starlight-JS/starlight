// TODO: Use mimalloc there?
use crate::prelude::*;
use std::{
    intrinsics::unlikely,
    mem::{size_of, ManuallyDrop},
    ptr::null_mut,
};

use super::{class::JsClass, object::TypedJsObject, Context};
pub struct JsArrayBuffer {
    pub(crate) data: *mut u8,
    pub(crate) attached: bool,
}

extern "C" fn drop_array_buffer(x: GcPointer<JsObject>) {
    unsafe {
        TypedJsObject::<JsArrayBuffer>::new(x).detach();
        ManuallyDrop::drop(x.data::<JsArrayBuffer>());
    }
}
/*
extern "C" fn array_buffer_serialize(x: &JsObject, serializer: &mut SnapshotSerializer) {
    let data = x.data::<JsArrayBuffer>();
    data.attached.serialize(serializer);
    let size = x.direct(JsArrayBuffer::BYTE_LENGTH_OFFSET).get_int32() as u32;
    size.serialize(serializer);
    if data.attached {
        assert!(!data.data.is_null());
        for i in 0..size {
            unsafe {
                data.data.add(i as _).read().serialize(serializer);
            }
        }
    }
}
extern "C" fn array_buffer_deserialize(x: &mut JsObject, deser: &mut Deserializer) {
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
        *x.direct_mut(JsArrayBuffer::BYTE_LENGTH_OFFSET) = JsValue::new(size as u32);
        *x.data::<JsArrayBuffer>() = ManuallyDrop::new(JsArrayBuffer {
            attached,
            data: buf,
        })
    }
}*/
extern "C" fn array_buffer_size() -> usize {
    size_of::<JsArrayBuffer>()
}

impl JsClass for JsArrayBuffer {
    fn class() -> &'static Class {
        define_jsclass!(
            JsArrayBuffer,
            ArrayBuffer,
            Some(drop_array_buffer),
            None,
            Some(array_buffer_size)
        )
    }
}

impl JsArrayBuffer {
    pub const BYTE_LENGTH_OFFSET: usize = 0;

    pub fn get_data_block(&self) -> *mut u8 {
        self.data
    }

    pub fn new(ctx: GcPointer<Context>) -> GcPointer<JsObject> {
        let structure = ctx.global_data().array_buffer_structure.unwrap();
        let mut this = JsObject::new(ctx, &structure, Self::class(), ObjectTag::ArrayBuffer);
        *this.data::<Self>() = ManuallyDrop::new(Self {
            data: null_mut(),
            attached: false,
        });

        *this.direct_mut(Self::BYTE_LENGTH_OFFSET) = JsValue::new(0u32);
        this
    }

    pub fn attached(&self) -> bool {
        self.attached
    }
    pub fn copy_data_block_bytes(
        dst: TypedJsObject<Self>,
        dst_index: usize,
        src: TypedJsObject<Self>,
        src_index: usize,
        count: usize,
    ) {
        if count == 0 {
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.get_data_block().add(src_index),
                dst.get_data_block().add(dst_index),
                count,
            )
        }
    }
}

impl TypedJsObject<JsArrayBuffer> {
    pub fn byte_length(&self) -> usize {
        self.object()
            .direct(JsArrayBuffer::BYTE_LENGTH_OFFSET)
            .get_int32() as u32 as usize
    }
    pub fn size(&self) -> usize {
        self.byte_length()
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
                self.set_size(0);
            }
        }
        self.attached = false;
    }
    pub unsafe fn set_size(&mut self, size: usize) {
        *self.object().direct_mut(JsArrayBuffer::BYTE_LENGTH_OFFSET) = JsValue::new(size as u32);
    }

    pub fn create_data_block(
        &mut self,
        ctx: GcPointer<Context>,
        size: usize,
        zero: bool,
    ) -> Result<(), JsValue> {
        self.detach();
        if size == 0 {
            self.attached = true;
            return Ok(());
        }

        if unlikely(size > u32::MAX as usize) {
            let msg = JsString::new(ctx, "Cannot allocate a data block for the ArrayBuffer");
            return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
        }
        unsafe {
            self.data = if zero {
                libc::calloc(1, size).cast()
            } else {
                libc::malloc(size).cast()
            };

            if unlikely(self.data.is_null()) {
                let msg = JsString::new(ctx, "Cannot allocate a data block for the ArrayBuffer");
                return Err(JsValue::new(JsRangeError::new(ctx, msg, None)));
            }
            self.attached = true;
            self.set_size(size);
        }
        Ok(())
    }
}
