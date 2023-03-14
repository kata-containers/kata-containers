use syn::spanned::Spanned;

pub fn async_body(block: Box<syn::Block>) -> syn::Result<proc_macro2::TokenStream> {
    Err(syn::Error::new(
        block.span(),
        "`#[snafu::report]` cannot be used with async functions in this version of Rust",
    ))
}
