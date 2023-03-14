use quote::quote;
use syn::{spanned::Spanned, Item, ItemFn, ReturnType, Signature};

// In versions of Rust before 1.39, we can't use the `.await` keyword
// in *our* source code, even if we never generate it in the *output*
// source code. Hiding it behind a conditionally-compiled module
// works.
#[cfg(not(feature = "rust_1_39"))]
mod no_async;
#[cfg(not(feature = "rust_1_39"))]
use no_async::async_body;
#[cfg(feature = "rust_1_39")]
mod yes_async;
#[cfg(feature = "rust_1_39")]
use yes_async::async_body;

pub fn body(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let item = syn::parse::<Item>(item)?;

    let f = match item {
        Item::Fn(f) => f,
        _ => {
            return Err(syn::Error::new(
                item.span(),
                "`#[snafu::report]` may only be used on functions",
            ))
        }
    };

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = f;

    let Signature {
        constness,
        asyncness,
        unsafety,
        abi,
        fn_token,
        ident,
        generics,
        paren_token: _,
        inputs,
        variadic,
        output,
    } = sig;

    let output_ty = match output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    let error_ty = quote! { <#output_ty as ::snafu::__InternalExtractErrorType>::Err };

    let output = if cfg!(feature = "rust_1_61") {
        quote! { -> ::snafu::Report<#error_ty> }
    } else {
        quote! { -> ::core::result::Result<(), ::snafu::Report<#error_ty>> }
    };

    let block = if asyncness.is_some() {
        async_body(block)?
    } else {
        if cfg!(feature = "rust_1_61") {
            quote! {
                {
                    let __snafu_body = || #block;
                    <::snafu::Report<_> as ::core::convert::From<_>>::from(__snafu_body())
                }
            }
        } else {
            quote! {
                {
                    let __snafu_body = || #block;
                    ::core::result::Result::map_err(__snafu_body(), ::snafu::Report::from_error)
                }
            }
        }
    };

    Ok(quote! {
        #(#attrs)*
        #vis
        #constness
        #asyncness
        #unsafety
        #abi
        #fn_token
        #ident
        #generics
        (#inputs #variadic)
        #output
        #block
    })
}
