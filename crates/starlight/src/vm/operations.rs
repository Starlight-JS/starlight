use crate::prelude::*;

pub fn normalize_prototype_chain(rt: &mut Runtime, base: &GcPointer<JsObject>) -> (usize, bool) {
    let mut saw_poly_proto = false;
    let mut count = 0;
    let stack = rt.shadowstack();
    letroot!(current = stack, *base);

    loop {
        let mut structure = current.structure;
        saw_poly_proto |= structure.has_poly_proto();
        let prototype = structure.stored_prototype(rt, &current);
        if prototype.is_null() {
            return (count, saw_poly_proto);
        }

        *current = prototype.get_jsobject();
        structure = current.structure;
        if structure.is_unique() {
            if structure.has_been_flattened_before {
                return (usize::MAX, saw_poly_proto);
            }
            structure.flatten_dictionary_structure(rt, &current);
        }
        count += 1;
    }
}
