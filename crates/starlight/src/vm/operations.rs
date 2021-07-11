/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::prelude::*;

pub fn normalize_prototype_chain(rt: &mut Runtime, base: &GcPointer<JsObject>) -> (usize, bool) {
    let mut saw_poly_proto = false;
    let mut count = 0;
    let stack = rt.shadowstack();
    letroot!(current = stack, *base);

    loop {
        let mut structure = current.structure;
        saw_poly_proto |= structure.get(rt).has_poly_proto();
        let prototype = structure.get(rt).stored_prototype(rt, &current);
        if prototype.is_null() {
            return (count, saw_poly_proto);
        }

        *current = prototype.get_jsobject();
        structure = current.structure;
        if structure.get(rt).is_unique() {
            if structure.get(rt).has_been_flattened_before {
                return (usize::MAX, saw_poly_proto);
            }
            structure.get(rt).flatten_dictionary_structure(rt, &current);
        }
        count += 1;
    }
}
