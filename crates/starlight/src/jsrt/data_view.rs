use std::{any::TypeId, mem::size_of};

use wtf_rs::swap_byte_order::SwapByteOrder;

use crate::{
    prelude::*,
    vm::{
        array_buffer::JsArrayBuffer, builder::Builtin, context::Context, data_view::JsDataView,
        object::TypedJsObject,
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

impl Builtin for JsDataView {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let obj_proto = ctx.global_data.object_prototype.unwrap();
        ctx.global_data.data_view_structure = Some(Structure::new_indexed(ctx, None, false));
        let proto_map = ctx
            .global_data
            .data_view_structure
            .unwrap()
            .change_prototype_transition(ctx, Some(obj_proto));
        let mut prototype = JsObject::new(ctx, &proto_map, JsObject::class(), ObjectTag::Ordinary);
        ctx.global_data
            .data_view_structure
            .unwrap()
            .change_prototype_with_no_transition(prototype);
        let mut constructor =
            JsNativeFunction::new(ctx, "DataView".intern(), data_view_constructor, 1);

        def_native_property!(ctx, constructor, prototype, prototype)?;
        def_native_property!(ctx, prototype, constructor, constructor)?;
        def_native_method!(ctx, prototype, getInt8, data_view_prototype_get::<i8>, 1)?;
        def_native_method!(ctx, prototype, getUint8, data_view_prototype_get::<u8>, 1)?;
        def_native_method!(ctx, prototype, getInt16, data_view_prototype_get::<i16>, 2)?;
        def_native_method!(ctx, prototype, getUint16, data_view_prototype_get::<u16>, 2)?;
        def_native_method!(ctx, prototype, getInt32, data_view_prototype_get::<i32>, 2)?;
        def_native_method!(ctx, prototype, getUint32, data_view_prototype_get::<u32>, 2)?;
        def_native_method!(
            ctx,
            prototype,
            getFloat64,
            data_view_prototype_get::<f64>,
            2
        )?;
        def_native_method!(
            ctx,
            prototype,
            getFloat32,
            data_view_prototype_get::<f32>,
            2
        )?;

        def_native_method!(ctx, prototype, setInt8, data_view_prototype_set::<i8>, 2)?;
        def_native_method!(ctx, prototype, setUint8, data_view_prototype_set::<u8>, 2)?;
        def_native_method!(ctx, prototype, setInt16, data_view_prototype_set::<i16>, 3)?;
        def_native_method!(ctx, prototype, setUint16, data_view_prototype_set::<u16>, 3)?;
        def_native_method!(ctx, prototype, setInt32, data_view_prototype_set::<i32>, 3)?;
        def_native_method!(ctx, prototype, setUint32, data_view_prototype_set::<u32>, 3)?;
        def_native_method!(
            ctx,
            prototype,
            setFloat64,
            data_view_prototype_set::<f64>,
            3
        )?;
        def_native_method!(
            ctx,
            prototype,
            setFloat32,
            data_view_prototype_set::<f32>,
            3
        )?;

        let byte_length =
            JsNativeFunction::new(ctx, "byteLength", data_view_prototype_byte_length, 0);
        def_native_getter!(ctx, prototype, byteLength, byte_length, NONE)?;

        let byte_offset =
            JsNativeFunction::new(ctx, "byteOffset", data_view_prototype_byte_offset, 0);
        def_native_getter!(ctx, prototype, byteOffset, byte_offset, NONE)?;

        let buffer = JsNativeFunction::new(ctx, "buffer".intern(), data_view_prototype_buffer, 0);
        def_native_getter!(ctx, prototype, buffer, buffer, NONE)?;

        ctx.global_data.data_view_prototype = Some(prototype);

        ctx.global_object()
            .put(ctx, "DataView".intern(), JsValue::new(constructor), false)?;
        Ok(())
    }
}
