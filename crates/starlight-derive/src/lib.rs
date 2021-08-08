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
    s.bind_with(|_bi| BindStyle::Ref);
    let trace_body = s.each(|bi| quote!(mark(#bi,tracer)));

    let trace_impl = s.gen_impl(quote! {

        gen impl Trace for @Self {
        #[inline] fn trace(&self,tracer: &mut Visitor) {
            #[allow(dead_code)]
            #[inline]
            fn mark<T: Trace + ?Sized>(it: & T,tracer: &mut Visitor) {
              it.trace(tracer);
                // Trace::trace(it,tracer);
            }
            match &*self { #trace_body }
        }
    }

    });
    quote! {
        #trace_impl
    }
}
