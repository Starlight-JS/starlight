/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::prelude::*;

use super::context::Context;

pub fn normalize_prototype_chain(ctx: &mut Context, base: &GcPointer<JsObject>) -> (usize, bool) {
    let mut saw_poly_proto = false;
    let mut count = 0;
    let stack = ctx.shadowstack();
    letroot!(current = stack, *base);

    loop {
        let mut structure = current.structure;
        saw_poly_proto |= structure.has_poly_proto();
        let prototype = structure.stored_prototype(ctx, &current);
        if prototype.is_null() {
            return (count, saw_poly_proto);
        }

        *current = prototype.get_jsobject();
        structure = current.structure;
        if structure.is_unique() {
            if structure.has_been_flattened_before {
                return (usize::MAX, saw_poly_proto);
            }
            structure.flatten_dictionary_structure(ctx, &current);
        }
        count += 1;
    }
}
