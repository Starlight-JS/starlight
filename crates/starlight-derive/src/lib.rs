/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use quote::quote;
use synstructure::{decl_derive, BindStyle, Structure};

decl_derive!([GcTrace, attributes(unsafe_ignore_trace)] => derive_trace);

fn derive_trace(mut s: Structure<'_>) -> proc_macro2::TokenStream {
    s.filter(|bi| {
        !bi.ast()
            .attrs
            .iter()
            .any(|attr| attr.path.is_ident("unsafe_ignore_trace"))
    });
    s.bind_with(|_bi| BindStyle::RefMut);
    let trace_body = s.each(|bi| quote!(mark(#bi,tracer)));

    let trace_impl = s.gen_impl(quote! {

        gen unsafe impl Trace for @Self {
        #[inline] fn trace(&mut self,tracer: &mut dyn Tracer) {
            #[allow(dead_code)]
            #[inline]
            fn mark<T: Trace + ?Sized>(it: &mut T,tracer: &mut dyn Tracer) {
              it.trace(tracer);
                // Trace::trace(it,tracer);
            }
            match &mut*self { #trace_body }
        }
    }

    });
    quote! {
        #trace_impl
    }
}
