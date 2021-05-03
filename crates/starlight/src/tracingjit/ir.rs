/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{gc::cell::GcPointer, prelude::Structure};

pub enum Ir {
    Swap,
    PushLiteral(u32),
    PushInt(i32),
    PushTrue,
    PushFalse,
    PushUndef,
    PushNull,
    PushNaN,
    GetFunction(u32),

    Call(u32),
    New(u32),

    CallBuiltin(u32, u32, u32),
}
