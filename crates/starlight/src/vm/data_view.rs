use super::method_table::MethodTable;
use super::{array_buffer::JsArrayBuffer, object::JsObject, Runtime};
use crate::gc::cell::{GcPointer, Trace, Tracer};
use crate::vm::object::ObjectTag;
use std::mem::ManuallyDrop;
use std::{
    intrinsics::copy_nonoverlapping,
    mem::{size_of, MaybeUninit},
};

pub struct JsDataView {
    /// buffer is the underlying storage of the bytes for a DataView.
    buffer: GcPointer<JsObject>,
    /// offset is the position within the buffer that the DataView begins at.
    offset: usize,
    /// length is the amount of bytes the DataView views inside the storage.
    length: usize,
}

impl JsDataView {
    pub fn set_buffer(&mut self, buffer: GcPointer<JsObject>, offset: usize, length: usize) {
        self.buffer = buffer;
        self.offset = offset;
        self.length = length;
    }
    pub fn attached(&self) -> bool {
        self.buffer.data::<JsArrayBuffer>().attached()
    }
    pub unsafe fn get<T: Copy>(&self, offset: usize, _little_endian: bool) -> T {
        assert!(self.attached(), "Cannot get on a detached buffer");
        assert!(
            offset + size_of::<T>() <= self.length,
            "Trying to read past the end of the buffer"
        );
        let mut result = MaybeUninit::<T>::uninit();
        copy_nonoverlapping(
            self.buffer
                .data::<JsArrayBuffer>()
                .get_data_block()
                .add(self.offset)
                .add(offset),
            result.as_mut_ptr().cast::<u8>(),
            size_of::<T>(),
        );
        // TODO: Reverse order of bytes
        result.assume_init()
    }

    pub unsafe fn set<T: Copy>(&self, offset: usize, value: T, _little_endian: bool) {
        assert!(self.attached(), "Cannot set on a detached buffer");
        assert!(
            offset + size_of::<T>() <= self.length,
            "Trying to write past the end of the buffer"
        );
        copy_nonoverlapping(
            &value as *const T as *const u8,
            self.buffer
                .data::<JsArrayBuffer>()
                .get_data_block()
                .add(self.offset)
                .add(offset),
            size_of::<T>(),
        );
    }

    pub fn byte_length(&self) -> usize {
        self.length
    }

    pub fn byte_offset(&self) -> usize {
        self.offset
    }

    pub fn new(
        rt: &mut Runtime,
        buffer: GcPointer<JsObject>,
        offset: usize,
        length: usize,
    ) -> GcPointer<JsObject> {
        assert!(
            buffer.is_class(JsArrayBuffer::get_class()),
            "Expected ArrayBuffer to create DataView object",
        );
        let map = rt.global_data().data_view_structure.unwrap();
        let mut obj = JsObject::new(rt, &map, Self::get_class(), ObjectTag::Ordinary);
        *obj.data::<Self>() = ManuallyDrop::new(Self {
            buffer,
            offset,
            length,
        });
        obj
    }

    define_jsclass_with_symbol!(
        JsObject,
        DataView,
        DataView,
        None,
        Some(trace_data_view),
        None,
        None,
        Some(data_view_size)
    );
}
// TODO: Deserialize and serialize data view.
#[allow(improper_ctypes_definitions)]
extern "C" fn trace_data_view(tracer: &mut dyn Tracer, obj: &mut JsObject) {
    obj.data::<JsDataView>().buffer.trace(tracer);
}

extern "C" fn data_view_size() -> usize {
    size_of::<JsDataView>()
}
