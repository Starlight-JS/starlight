use crate::{
    prelude::*,
    vm::{array_buffer::JsArrayBuffer, context::Context},
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

    let buf = this.data::<JsArrayBuffer>();
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

    let buf = this.data::<JsArrayBuffer>();
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
    let buf = this.data::<JsArrayBuffer>();
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
    let new_buf = JsArrayBuffer::new(ctx);
    new_buf
        .data::<JsArrayBuffer>()
        .create_data_block(ctx, new_len, true)?;

    todo!()
}

impl GcPointer<Context> {
    pub(crate) fn init_array_buffer_in_global_object(mut self) -> Result<(), JsValue> {
        let mut proto = self.global_data.array_buffer_prototype.unwrap();
        let constructor = proto
            .get_own_property(self, "constructor".intern())
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

    pub(crate) fn init_array_buffer_in_global_data(mut self) {
        // Do not care about GC since no GC is possible when initializing runtime.
        let mut init = || -> Result<(), JsValue> {
            let mut structure = Structure::new_indexed(
                self,
                Some(self.global_data.object_prototype.unwrap()),
                false,
            );
            let mut proto = JsObject::new(
                self,
                &structure,
                JsArrayBuffer::get_class(),
                ObjectTag::ArrayBuffer,
            );
            *proto.data::<JsArrayBuffer>() = std::mem::ManuallyDrop::new(JsArrayBuffer {
                data: std::ptr::null_mut(),
                size: 0,
                attached: false,
            });
            let map = structure.change_prototype_transition(self, Some(proto));
            self.global_data.array_buffer_prototype = Some(proto);
            self.global_data.array_buffer_structure = Some(map);

            let mut ctor =
                JsNativeFunction::new(self, "ArrayBuffer".intern(), array_buffer_constructor, 1);
            ctor.put(self, "prototype".intern(), JsValue::new(proto), false)?;
            proto.put(self, "constructor".intern(), JsValue::new(ctor), false)?;
            let byte_length =
                JsNativeFunction::new(self, "byteLength".intern(), array_buffer_byte_length, 0);
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
            //def_native_method!(ctx, proto, byteLength, array_buffer_byte_length, 0)?;
            def_native_method!(self, proto, slice, array_buffer_slice, 2)?;
            Ok(())
        };

        match init() {
            Ok(_) => {}
            Err(e) => {
                unreachable!(
                    "Failed to initialize ArrayBuffer: '{}'",
                    e.to_string(self).unwrap_or_else(|_| unreachable!())
                );
            }
        }
    }
}
