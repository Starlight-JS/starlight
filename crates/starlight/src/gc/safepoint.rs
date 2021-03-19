use crate::heap::cell::Trace;

#[repr(C)]
#[derive(Debug)]
pub struct ShadowStackLink {
    pub element: *const dyn Trace,
    pub prev: *const ShadowStackLink,
}

#[derive(Clone)]
pub struct ShadowStack {
    pub(crate) last: *const ShadowStackLink,
}
