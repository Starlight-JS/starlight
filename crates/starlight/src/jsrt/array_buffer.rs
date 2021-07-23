use crate::{
    prelude::*,
    vm::{
        array_buffer::JsArrayBuffer, builder::Builtin, context::Context, object::TypedJsObject,
        structure_builder::StructureBuilder,
    },
};
pub fn array_buffer_constructor(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    if !args.ctor_call {
        return Err(JsValue::new(ctx.new_type_error(
            "ArrayBuffer() called in function context instead of constructor",
        )));
    }
    let stack = ctx.shadowstack();
    letroot!(this = stack, JsArrayBuffer::new(ctx));

    let mut buf = TypedJsObject::<JsArrayBuffer>::new(*this);
    let length = args.at(0).to_int32(ctx)?;
    assert!(
        !buf.attached(),
        "A new array buffer should not have an existing buffer"
    );
    buf.create_data_block(ctx, length as _, true)?;
    Ok(JsValue::new(*this))
}

pub fn array_buffer_byte_length(
    ctx: GcPointer<Context>,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(this = stack, args.this.to_object(ctx)?);
    if !this.is_class(JsArrayBuffer::class()) {
        return Err(JsValue::new(
            ctx.new_type_error("ArrayBuffer.prototype.byteLength is not generic"),
        ));
    }

    let buf = TypedJsObject::<JsArrayBuffer>::new(*this);
    Ok(JsValue::new(buf.size() as u32))
}

pub fn array_buffer_slice(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = ctx.shadowstack();
    letroot!(this = stack, args.this.to_object(ctx)?);
    if !this.is_class(JsArrayBuffer::class()) {
        return Err(JsValue::new(
            ctx.new_type_error("ArrayBuffer.prototype.slice is not generic"),
        ));
    }
    let buf = TypedJsObject::<JsArrayBuffer>::new(*this);
    let start = args.at(0).to_int32(ctx)?;
    let end = args.at(1).to_int32(ctx)?;
    let len = buf.size();
    let relative_start = start;
    let first = if relative_start <= 0 {
        std::cmp::max((len as i32 + relative_start) as usize, 0)
    } else {
        std::cmp::min(relative_start as usize, len)
    };

    let relative_end;
    if args.at(1).is_undefined() {
        relative_end = len as i64;
    } else {
        relative_end = end as i64;
    }

    let finale = if relative_end < 0 {
        std::cmp::max(len as i64 + relative_end, 0) as usize
    } else {
        std::cmp::min(relative_end, len as i64) as usize
    };
    let new_len = std::cmp::max(finale as i64 - first as i64, 0) as usize;
    let mut new_buf = TypedJsObject::<JsArrayBuffer>::new(JsArrayBuffer::new(ctx));
    new_buf.create_data_block(ctx, new_len, true)?;
    // 17. If new does not have an [[ArrayBufferData]] internal slot, throw a
    // TypeError exception.
    // 18. If IsDetachedBuffer(new) is true, throw a TypeError exception.
    // 19. If SameValue(new, O) is true, throw a TypeError exception.
    // 20. If the value of new’s [[ArrayBufferByteLength]] internal
    // slot < newLen, throw a TypeError exception.
    // 21. NOTE: Side-effects of the above steps may have detached O.
    // 22. If IsDetachedBuffer(O) is true, throw a TypeError exception.
    if !buf.attached() || !new_buf.attached() {
        return Err(JsValue::new(
            ctx.new_type_error("Cannot split with detached ArrayBuffers"),
        ));
    }
    JsArrayBuffer::copy_data_block_bytes(new_buf, 0, buf, first, new_len);

    Ok(JsValue::new(new_buf))
}

impl Builtin for JsArrayBuffer {
    fn init(mut ctx: GcPointer<Context>) -> Result<(), JsValue> {
        let mut builder = StructureBuilder::new(None);
        assert_eq!(
            builder
                .add("byteLength".intern(), create_data(AttrExternal::new(None)))
                .offset,
            0
        );
        let mut structure = builder.build(ctx, false, false);
        let proto_map = structure
            .change_prototype_transition(ctx, Some(ctx.global_data().object_prototype.unwrap()));
        let mut prototype = JsObject::new(
            ctx,
            &proto_map,
            JsArrayBuffer::class(),
            ObjectTag::ArrayBuffer,
        );

        structure.change_prototype_with_no_transition(prototype);
        *prototype.data::<JsArrayBuffer>() = std::mem::ManuallyDrop::new(JsArrayBuffer {
            data: std::ptr::null_mut(),

            attached: false,
        });

        ctx.global_data.array_buffer_prototype = Some(prototype);
        ctx.global_data.array_buffer_structure = Some(structure);

        let mut constructor =
            JsNativeFunction::new(ctx, "ArrayBuffer".intern(), array_buffer_constructor, 1);

        def_native_property!(ctx, constructor, prototype, prototype)?;
        def_native_property!(ctx, prototype, constructor, constructor)?;
        def_native_method!(ctx, prototype, slice, array_buffer_slice, 2)?;

        ctx.global_object().put(
            ctx,
            "ArrayBuffer".intern(),
            JsValue::new(constructor),
            false,
        )?;
        Ok(())
    }
}
