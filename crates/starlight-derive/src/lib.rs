use quote::quote;
use synstructure::{decl_derive, Structure};

decl_derive!([Trace, attributes(unsafe_ignore_trace)] => derive_trace);

fn derive_trace(mut s: Structure<'_>) -> proc_macro2::TokenStream {
    s.filter(|bi| {
        !bi.ast()
            .attrs
            .iter()
            .any(|attr| attr.path.is_ident("unsafe_ignore_trace"))
    });
    let trace_body = s.each(|bi| quote!(mark(#bi,tracer)));

    let trace_impl = s.gen_impl(quote! {

        gen unsafe impl Trace for @Self {
        #[inline] fn trace(&self,tracer: &mut dyn Tracer) {
            #[allow(dead_code)]
            #[inline]
            fn mark<T: Trace + ?Sized>(it: &T,tracer: &mut dyn Tracer) {
                Trace::trace(it,tracer);
            }
            match *self { #trace_body }
        }
    }

    });
    quote! {
        #trace_impl
    }
}
