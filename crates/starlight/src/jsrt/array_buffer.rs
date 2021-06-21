use crate::{prelude::*, vm::array_buffer::JsArrayBuffer};
pub fn array_buffer_constructor(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    if !args.ctor_call {
        return Err(JsValue::new(rt.new_type_error(
            "ArrayBuffer() called in function context instead of constructor",
        )));
    }
    let stack = rt.shadowstack();
    letroot!(this = stack, JsArrayBuffer::new(rt));

    let buf = this.data::<JsArrayBuffer>();
    let length = args.at(0).to_int32(rt)?;
    assert!(
        !buf.attached(),
        "A new array buffer should not have an existing buffer"
    );
    buf.create_data_block(rt, length as _, true)?;
    Ok(JsValue::new(*this))
}

pub fn array_buffer_byte_length(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    letroot!(this = stack, args.this.to_object(rt)?);
    if !this.is_class(JsArrayBuffer::get_class()) {
        return Err(JsValue::new(
            rt.new_type_error("ArrayBuffer.prototype.byteLength is not generic"),
        ));
    }

    let buf = this.data::<JsArrayBuffer>();
    Ok(JsValue::new(buf.size() as u32))
}

pub fn array_buffer_slice(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    letroot!(this = stack, args.this.to_object(rt)?);
    if !this.is_class(JsArrayBuffer::get_class()) {
        return Err(JsValue::new(
            rt.new_type_error("ArrayBuffer.prototype.slice is not generic"),
        ));
    }
    let buf = this.data::<JsArrayBuffer>();
    let start = args.at(0).to_int32(rt)?;
    let end = args.at(1).to_int32(rt)?;
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
    let new_buf = JsArrayBuffer::new(rt);
    new_buf
        .data::<JsArrayBuffer>()
        .create_data_block(rt, new_len, true)?;

    todo!()
}

pub(crate) fn array_buffer_init(rt: &mut Runtime) {
    let mut init = || -> Result<(), JsValue> {
        let mut structure =
            Structure::new_indexed(rt, Some(rt.global_data.object_prototype.unwrap()), false);
        let mut proto = JsObject::new(
            rt,
            &structure,
            JsArrayBuffer::get_class(),
            ObjectTag::ArrayBuffer,
        );
        let map = structure.change_prototype_transition(rt, Some(proto));
        rt.global_data.array_buffer_prototype = Some(proto);
        rt.global_data.array_buffer_structure = Some(map);

        let mut ctor =
            JsNativeFunction::new(rt, "ArrayBuffer".intern(), array_buffer_constructor, 1);
        ctor.put(rt, "prototype".intern(), JsValue::new(proto), false)?;
        proto.put(rt, "constructor".intern(), JsValue::new(ctor), false)?;
        let byte_length =
            JsNativeFunction::new(rt, "byteLength".intern(), array_buffer_byte_length, 0);
        proto.define_own_property(
            rt,
            "byteLength".intern(),
            &*AccessorDescriptor::new(
                JsValue::new(byte_length),
                JsValue::encode_undefined_value(),
                NONE,
            ),
            false,
        )?;
        //def_native_method!(rt, proto, byteLength, array_buffer_byte_length, 0)?;
        def_native_method!(rt, proto, slice, array_buffer_slice, 2)?;
        rt.global_object()
            .put(rt, "ArrayBuffer".intern(), JsValue::new(ctor), false)?;
        Ok(())
    };

    match init() {
        Ok(_) => {}
        Err(e) => {
            unreachable!(
                "Failed to initialize ArrayBuffer: '{}'",
                e.to_string(rt).unwrap_or_else(|_| unreachable!())
            );
        }
    }
}
