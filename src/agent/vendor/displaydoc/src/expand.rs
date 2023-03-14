use super::attr::AttrsHelper;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Result};

pub(crate) fn derive(input: &DeriveInput) -> Result<TokenStream> {
    let impls = match &input.data {
        Data::Struct(data) => impl_struct(input, data),
        Data::Enum(data) => impl_enum(input, data),
        Data::Union(_) => Err(Error::new_spanned(input, "Unions are not supported")),
    }?;

    let helpers = specialization();
    let dummy_const = format_ident!("_DERIVE_Display_FOR_{}", input.ident);
    Ok(quote! {
        #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
        const #dummy_const: () = {
            #helpers
            #impls
        };
    })
}

#[cfg(feature = "std")]
fn specialization() -> TokenStream {
    quote! {
        trait DisplayToDisplayDoc {
            fn __displaydoc_display(&self) -> Self;
        }

        impl<T: core::fmt::Display> DisplayToDisplayDoc for &T {
            fn __displaydoc_display(&self) -> Self {
                self
            }
        }

        // If the `std` feature gets enabled we want to ensure that any crate
        // using displaydoc can still reference the std crate, which is already
        // being compiled in by whoever enabled the `std` feature in
        // `displaydoc`, even if the crates using displaydoc are no_std.
        extern crate std;

        trait PathToDisplayDoc {
            fn __displaydoc_display(&self) -> std::path::Display<'_>;
        }

        impl PathToDisplayDoc for std::path::Path {
            fn __displaydoc_display(&self) -> std::path::Display<'_> {
                self.display()
            }
        }

        impl PathToDisplayDoc for std::path::PathBuf {
            fn __displaydoc_display(&self) -> std::path::Display<'_> {
                self.display()
            }
        }
    }
}

#[cfg(not(feature = "std"))]
fn specialization() -> TokenStream {
    quote! {}
}

fn impl_struct(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let helper = AttrsHelper::new(&input.attrs);

    let display = helper.display(&input.attrs)?.map(|display| {
        let pat = match &data.fields {
            Fields::Named(fields) => {
                let var = fields.named.iter().map(|field| &field.ident);
                quote!(Self { #(#var),* })
            }
            Fields::Unnamed(fields) => {
                let var = (0..fields.unnamed.len()).map(|i| format_ident!("_{}", i));
                quote!(Self(#(#var),*))
            }
            Fields::Unit => quote!(_),
        };
        quote! {
            impl #impl_generics core::fmt::Display for #ty #ty_generics #where_clause {
                fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                    #[allow(unused_variables)]
                    let #pat = self;
                    #display
                }
            }
        }
    });

    Ok(quote! { #display })
}

fn impl_enum(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    let ty = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let helper = AttrsHelper::new(&input.attrs);

    let displays = data
        .variants
        .iter()
        .map(|variant| helper.display_with_input(&input.attrs, &variant.attrs))
        .collect::<Result<Vec<_>>>()?;

    if displays.iter().any(Option::is_some) {
        let arms = data
            .variants
            .iter()
            .zip(displays)
            .map(|(variant, display)| {
                let display =
                    display.ok_or_else(|| Error::new_spanned(variant, "missing doc comment"))?;
                let ident = &variant.ident;
                Ok(match &variant.fields {
                    Fields::Named(fields) => {
                        let var = fields.named.iter().map(|field| &field.ident);
                        quote!(#ty::#ident { #(#var),* } => { #display })
                    }
                    Fields::Unnamed(fields) => {
                        let var = (0..fields.unnamed.len()).map(|i| format_ident!("_{}", i));
                        quote!(#ty::#ident(#(#var),*) => { #display })
                    }
                    Fields::Unit => quote!(#ty::#ident => { #display }),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(quote! {
            impl #impl_generics core::fmt::Display for #ty #ty_generics #where_clause {
                fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                    #[allow(unused_variables)]
                    match self {
                        #(#arms,)*
                    }
                }
            }
        })
    } else {
        Err(Error::new_spanned(input, "Missing doc comments"))
    }
}
