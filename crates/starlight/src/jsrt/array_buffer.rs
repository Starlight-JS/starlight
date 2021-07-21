use crate::{
    prelude::*,
    vm::{
        array_buffer::JsArrayBuffer, context::Context, object::TypedJsObject,
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
    if !this.is_class(JsArrayBuffer::get_class()) {
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
    if !this.is_class(JsArrayBuffer::get_class()) {
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

impl GcPointer<Context> {
    pub(crate) fn init_array_buffer_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.array_buffer_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, S_CONSTURCTOR.intern())
            .unwrap()
            .value();
        self.global_object().put(
            self,
            "ArrayBuffer".intern(),
            JsValue::new(constructor),
            false,
        )?;
        Ok(())
    }

    pub(crate) fn init_array_buffer_in_global_data(mut self) -> Result<(), JsValue> {
        // Do not care about GC since no GC is possible when initializing runtime.

        let mut builder = StructureBuilder::new(None);
        assert_eq!(
            builder
                .add("byteLength".intern(), create_data(AttrExternal::new(None)))
                .offset,
            0
        );
        let mut structure = builder.build(self, false, false);
        let proto_map = structure
            .change_prototype_transition(self, Some(self.global_data().object_prototype.unwrap()));
        let mut proto = JsObject::new(
            self,
            &proto_map,
            JsArrayBuffer::get_class(),
            ObjectTag::ArrayBuffer,
        );

        structure.change_prototype_with_no_transition(proto);
        *proto.data::<JsArrayBuffer>() = std::mem::ManuallyDrop::new(JsArrayBuffer {
            data: std::ptr::null_mut(),

            attached: false,
        });

        self.global_data.array_buffer_prototype = Some(proto);
        self.global_data.array_buffer_structure = Some(structure);

        let mut ctor =
            JsNativeFunction::new(self, "ArrayBuffer".intern(), array_buffer_constructor, 1);

        def_native_property!(self, ctor, prototype, proto)?;
        def_native_property!(self, proto, constructor, ctor)?;
        def_native_method!(self, proto, slice, array_buffer_slice, 2)?;
        Ok(())
    }
}
