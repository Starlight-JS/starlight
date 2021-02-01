use super::{js_value::JsValue, storage::FixedStorage};

pub type ObjectSlots = FixedStorage<JsValue>;
pub struct JsObject {}
