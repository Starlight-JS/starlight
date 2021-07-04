use crate::{prelude::*, vm::data_view::JsDataView};
pub fn data_view_prototype_buffer(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if !this.is_class(JsDataView::get_class()) {
        return Err(JsValue::new(rt.new_type_error(
            "DataView.prototype.buffer called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().get_buffer()))
}
pub fn data_view_prototype_byte_offset(
    rt: &mut Runtime,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if !this.is_class(JsDataView::get_class()) {
        return Err(JsValue::new(rt.new_type_error(
            "DataView.prototype.byteOffset called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().byte_offset() as u32))
}
pub fn data_view_prototype_byte_length(
    rt: &mut Runtime,
    args: &Arguments,
) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if !this.is_class(JsDataView::get_class()) {
        return Err(JsValue::new(rt.new_type_error(
            "DataView.prototype.byteLength called on a non DataView object",
        )));
    }
    Ok(JsValue::new(this.data::<JsDataView>().byte_length() as u32))
}
