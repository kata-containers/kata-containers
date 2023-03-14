use quote::quote;

pub fn async_body(block: Box<syn::Block>) -> syn::Result<proc_macro2::TokenStream> {
    if cfg!(feature = "rust_1_61") {
        Ok(quote! {
            {
                let __snafu_body = async #block;
                <::snafu::Report<_> as ::core::convert::From<_>>::from(__snafu_body.await)
            }
        })
    } else {
        Ok(quote! {
            {
                let __snafu_body = async #block;
                ::core::result::Result::map_err(__snafu_body.await, ::snafu::Report::from_error)
            }
        })
    }
}
