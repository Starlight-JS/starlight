use std::{any::TypeId, mem::size_of};

use wtf_rs::swap_byte_order::SwapByteOrder;

use crate::{
    constant::S_CONSTURCTOR,
    prelude::*,
    vm::{
        array_buffer::JsArrayBuffer, context::Context, data_view::JsDataView, object::TypedJsObject,
    },
    JsTryFrom,
};
pub fn data_view_prototype_buffer(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if !this.is_class(JsDataView::class()) {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.buffer called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().get_buffer()))
}
pub fn data_view_prototype_byte_offset(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if !this.is_class(JsDataView::class()) {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.byteOffset called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().byte_offset() as u32))
}
pub fn data_view_prototype_byte_length(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if !this.is_class(JsDataView::class()) {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.byteLength called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().byte_length() as u32))
}

pub fn data_view_prototype_get<T: SwapByteOrder + Into<JsValue> + Copy>(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(ctx)?;
    if !this.is_class(JsDataView::class()) {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.get<T> called on a non DataView object",
        )));
    }

    let res = super::to_index(ctx, args.at(0))?;
    let byte_offset = res as usize;
    let little_endian = args.at(1).to_boolean();

    if !this.data::<JsDataView>().attached() {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.get<T> called on a detached ArrayBuffer",
        )));
    }

    if byte_offset + size_of::<T>() > this.data::<JsDataView>().byte_length() {
        return Err(JsValue::new(ctx.new_range_error(format!(
            "DataView.prototype.get<T>(): Cannot read that many bytes {}",
            byte_offset + size_of::<T>()
        ))));
    }
    Ok(unsafe {
        this.data::<JsDataView>()
            .get::<T>(byte_offset, little_endian)
            .into()
    })
}

pub fn data_view_prototype_set<T: SwapByteOrder + Into<JsValue> + Copy + 'static>(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = TypedJsObject::<JsDataView>::try_from(ctx, args.this)?;

    let res = super::to_index(ctx, args.at(0))?;
    let byte_offset = res as usize;
    let little_endian = args.at(2).to_boolean();

    if !this.attached() {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView.prototype.set<T> called on a detached ArrayBuffer",
        )));
    }

    if byte_offset + size_of::<T>() > this.byte_length() {
        return Err(JsValue::new(ctx.new_range_error(format!(
            "DataView.prototype.set<T>(): Cannot write that many bytes {}",
            byte_offset + size_of::<T>()
        ))));
    }

    let num = args.at(1).to_number(ctx)?;
    unsafe {
        if TypeId::of::<u8>() == TypeId::of::<T>() {
            let dest = num.clamp(u8::MIN as _, u8::MAX as _);
            this.set::<u8>(byte_offset, dest as _, little_endian);
        } else if TypeId::of::<f64>() == TypeId::of::<T>() {
            this.set::<f64>(byte_offset, num, little_endian);
        } else if TypeId::of::<f32>() == TypeId::of::<T>() {
            this.set::<f32>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<i64>() == TypeId::of::<T>() {
            this.set::<i64>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<u64>() == TypeId::of::<T>() {
            this.set::<u64>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<u32>() == TypeId::of::<T>() {
            this.set::<u32>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<u16>() == TypeId::of::<T>() {
            this.set::<f64>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<i32>() == TypeId::of::<T>() {
            this.set::<f64>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<i16>() == TypeId::of::<T>() {
            this.set::<f64>(byte_offset, num as _, little_endian);
        } else if TypeId::of::<i8>() == TypeId::of::<T>() {
            this.set::<f64>(byte_offset, num as _, little_endian);
        } else {
            unreachable!();
        }
    }
    Ok(JsValue::encode_undefined_value())
}

pub fn data_view_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if !args.ctor_call {
        return Err(JsValue::new(ctx.new_type_error(
            "DataView() called in a function context instead of constructor",
        )));
    }

    let buffer = args.at(0).to_object(ctx).ok().and_then(|object| {
        if object.is_class(JsArrayBuffer::class()) {
            Some(object)
        } else {
            None
        }
    });
    let buffer = TypedJsObject::<JsArrayBuffer>::new(if buffer.is_none() {
        return Err(JsValue::new(ctx.new_type_error(
            "new DataView(buffer, [byteOffset], [byteLength]): buffer must be an ArrayBuffer",
        )));
    } else {
        buffer.unwrap()
    });
    let byte_length = args.at(2);
    let res = super::to_index(ctx, args.at(1))?;
    let offset = res as usize;
    let buffer_length = buffer.size();
    if offset > buffer_length {
        return Err(JsValue::new(ctx.new_range_error("new DataView(buffer, [byteOffset], byteLength]): byteOffset must be <= the buffer's byte length")));
    }
    let view_byte_length;
    if byte_length.is_undefined() {
        view_byte_length = buffer_length - offset;
    } else {
        let res = super::to_index(ctx, byte_length)?;
        view_byte_length = res as _;
        if offset + view_byte_length > buffer_length {
            return Err(JsValue::new(ctx.new_range_error("new DataView(buffer, [byteOffset], byteLength]): byteOffset + byteLength must be <= the buffer's byte length")));
        }
    }

    let this = JsDataView::new(ctx, buffer, offset, view_byte_length);
    Ok(JsValue::new(this))
}

impl GcPointer<Context> {
    pub(crate) fn init_data_view_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.data_view_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();
        self.global_object()
            .put(self, "DataView".intern(), JsValue::new(constructor), false)?;
        Ok(())
    }

    pub(crate) fn init_data_view_in_global_data(mut self) -> Result<(), JsValue> {
        let obj_proto = self.global_data.object_prototype.unwrap();
        self.global_data.data_view_structure = Some(Structure::new_indexed(self, None, false));
        let proto_map = self
            .global_data
            .data_view_structure
            .unwrap()
            .change_prototype_transition(self, Some(obj_proto));
        let mut proto = JsObject::new(self, &proto_map, JsObject::class(), ObjectTag::Ordinary);
        self.global_data
            .data_view_structure
            .unwrap()
            .change_prototype_with_no_transition(proto);
        let mut ctor = JsNativeFunction::new(self, "DataView".intern(), data_view_constructor, 1);

        def_native_property!(self, ctor, prototype, proto)?;
        def_native_property!(self, proto, constructor, ctor)?;
        def_native_method!(self, proto, getInt8, data_view_prototype_get::<i8>, 1)?;
        def_native_method!(self, proto, getUint8, data_view_prototype_get::<u8>, 1)?;
        def_native_method!(self, proto, getInt16, data_view_prototype_get::<i16>, 2)?;
        def_native_method!(self, proto, getUint16, data_view_prototype_get::<u16>, 2)?;
        def_native_method!(self, proto, getInt32, data_view_prototype_get::<i32>, 2)?;
        def_native_method!(self, proto, getUint32, data_view_prototype_get::<u32>, 2)?;
        def_native_method!(self, proto, getFloat64, data_view_prototype_get::<f64>, 2)?;
        def_native_method!(self, proto, getFloat32, data_view_prototype_get::<f32>, 2)?;

        def_native_method!(self, proto, setInt8, data_view_prototype_set::<i8>, 2)?;
        def_native_method!(self, proto, setUint8, data_view_prototype_set::<u8>, 2)?;
        def_native_method!(self, proto, setInt16, data_view_prototype_set::<i16>, 3)?;
        def_native_method!(self, proto, setUint16, data_view_prototype_set::<u16>, 3)?;
        def_native_method!(self, proto, setInt32, data_view_prototype_set::<i32>, 3)?;
        def_native_method!(self, proto, setUint32, data_view_prototype_set::<u32>, 3)?;
        def_native_method!(self, proto, setFloat64, data_view_prototype_set::<f64>, 3)?;
        def_native_method!(self, proto, setFloat32, data_view_prototype_set::<f32>, 3)?;

        let byte_length = JsNativeFunction::new(
            self,
            "byteLength".intern(),
            data_view_prototype_byte_length,
            0,
        );
        proto.define_own_property(
            self,
            "byteLength".intern(),
            &*AccessorDescriptor::new(
                JsValue::new(byte_length),
                JsValue::encode_undefined_value(),
                NONE,
            ),
            false,
        )?;
        let byte_offset = JsNativeFunction::new(
            self,
            "byteOffset".intern(),
            data_view_prototype_byte_offset,
            0,
        );
        proto.define_own_property(
            self,
            "byteOffset".intern(),
            &*AccessorDescriptor::new(
                JsValue::new(byte_offset),
                JsValue::encode_undefined_value(),
                NONE,
            ),
            false,
        )?;
        let buffer = JsNativeFunction::new(self, "buffer".intern(), data_view_prototype_buffer, 0);
        proto.define_own_property(
            self,
            "buffer".intern(),
            &*AccessorDescriptor::new(
                JsValue::new(buffer),
                JsValue::encode_undefined_value(),
                NONE,
            ),
            false,
        )?;

        self.global_data.data_view_prototype = Some(proto);
        Ok(())
    }
}
