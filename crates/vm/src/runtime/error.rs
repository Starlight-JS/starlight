use std::fmt::{Debug, Formatter};

use super::js_value::JsValue;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ErrorCode {
    Normal,
    Eval,
    Range,
    Reference,
    Syntax,
    Type,
    URI,
    User,
}

pub struct Error {
    pub value: JsValue,
    pub detail: String,
    code: ErrorCode,
}

impl Error {
    pub fn clear(&mut self) {
        self.detail.clear();
        self.value = JsValue::undefined();
    }

    pub fn value(&self) -> JsValue {
        self.value
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn report_str(code: ErrorCode, s: &str) -> Box<Self> {
        Box::new(Self {
            detail: s.to_string(),
            value: JsValue::undefined(),
            code,
        })
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {:?}: {}", self.code, self.detail)
    }
}
