use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Error,
    Meta::{NameValue, Path},
    NestedMeta::Meta,
    Type, TypePath,
};

use crate::utils::*;

pub fn expand_type_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let name = match input.data {
        Data::Struct(_) => input.ident,
        _ => return Err(Error::new(input.span(), "only structs supported")),
    };

    let zv = zvariant_path();
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #zv::Type for #name #ty_generics
        #where_clause
        {
            fn signature() -> #zv::Signature<'static> {
                #zv::Signature::from_static_str_unchecked("a{sv}")
            }
        }
    })
}

pub fn expand_serialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let (name, data) = match input.data {
        Data::Struct(data) => (input.ident, data),
        _ => return Err(Error::new(input.span(), "only structs supported")),
    };

    let zv = zvariant_path();
    let mut entries = quote! {};

    for f in &data.fields {
        let name = &f.ident;
        let dict_name = get_rename_attribute(&f.attrs, f.span())?
            .unwrap_or_else(|| f.ident.as_ref().unwrap().to_string());

        let is_option = match &f.ty {
            Type::Path(TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => segments.last().unwrap().ident == "Option",
            _ => false,
        };

        let e = if is_option {
            quote! {
                if self.#name.is_some() {
                    map.serialize_entry(#dict_name, &#zv::SerializeValue(self.#name.as_ref().unwrap()))?;
                }
            }
        } else {
            quote! {
                map.serialize_entry(#dict_name, &#zv::SerializeValue(&self.#name))?;
            }
        };

        entries.extend(e);
    }

    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #zv::export::serde::ser::Serialize for #name #ty_generics
        #where_clause
        {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #zv::export::serde::ser::Serializer,
            {
                use #zv::export::serde::ser::SerializeMap;

                // zbus doesn't care about number of entries (it would need bytes instead)
                let mut map = serializer.serialize_map(::std::option::Option::None)?;
                #entries
                map.end()
            }
        }
    })
}

pub fn expand_deserialize_derive(input: DeriveInput) -> Result<TokenStream, Error> {
    let (name, data) = match input.data {
        Data::Struct(data) => (input.ident, data),
        _ => return Err(Error::new(input.span(), "only structs supported")),
    };

    let mut deny_unknown_fields = false;
    for meta_item in input.attrs.iter().flat_map(get_meta_items).flatten() {
        match &meta_item {
            Meta(Path(p)) if p.is_ident("deny_unknown_fields") => {
                deny_unknown_fields = true;
            }
            Meta(NameValue(name_val)) => {
                match name_val
                    .path
                    .get_ident()
                    .map(ToString::to_string)
                    .unwrap_or_default()
                    .as_str()
                {
                    "signature" => continue,
                    _ => return Err(Error::new(meta_item.span(), "unsupported attribute")),
                }
            }
            _ => return Err(Error::new(meta_item.span(), "unsupported attribute")),
        }
    }

    let visitor = format_ident!("{}Visitor", name);
    let zv = zvariant_path();
    let mut fields = Vec::new();
    let mut req_fields = Vec::new();
    let mut dict_names = Vec::new();
    let mut entries = Vec::new();

    for f in &data.fields {
        let name = &f.ident;
        let dict_name = get_rename_attribute(&f.attrs, f.span())?
            .unwrap_or_else(|| f.ident.as_ref().unwrap().to_string());

        let is_option = match &f.ty {
            Type::Path(TypePath {
                path: syn::Path { segments, .. },
                ..
            }) => segments.last().unwrap().ident == "Option",
            _ => false,
        };

        entries.push(quote! {
            #dict_name => {
                // FIXME: add an option about strict parsing (instead of silently skipping the field)
                #name = access.next_value::<#zv::DeserializeValue<_>>().map(|v| v.0).ok();
            }
        });

        dict_names.push(dict_name);
        fields.push(name);

        if !is_option {
            req_fields.push(name);
        }
    }

    let fallback = if deny_unknown_fields {
        quote! {
            field => {
                return ::std::result::Result::Err(
                    <M::Error as #zv::export::serde::de::Error>::unknown_field(
                        field,
                        &[#(#dict_names),*],
                    ),
                );
            }
        }
    } else {
        quote! {
            unknown => {
                let _ = access.next_value::<#zv::Value>();
            }
        }
    };
    entries.push(fallback);

    let (_, ty_generics, _) = input.generics.split_for_impl();
    let mut generics = input.generics.clone();
    let def = syn::LifetimeDef {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'de", Span::call_site()),
        colon_token: None,
        bounds: Punctuated::new(),
    };
    generics.params = Some(syn::GenericParam::Lifetime(def))
        .into_iter()
        .chain(generics.params)
        .collect();

    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #zv::export::serde::de::Deserialize<'de> for #name #ty_generics
        #where_clause
        {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: #zv::export::serde::de::Deserializer<'de>,
            {
                struct #visitor #ty_generics(::std::marker::PhantomData<#name #ty_generics>);

                impl #impl_generics #zv::export::serde::de::Visitor<'de> for #visitor #ty_generics {
                    type Value = #name #ty_generics;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str("a dictionary")
                    }

                    fn visit_map<M>(
                        self,
                        mut access: M,
                    ) -> ::std::result::Result<Self::Value, M::Error>
                    where
                        M: #zv::export::serde::de::MapAccess<'de>,
                    {
                        #( let mut #fields = ::std::default::Default::default(); )*

                        // does not check duplicated fields, since those shouldn't exist in stream
                        while let ::std::option::Option::Some(key) = access.next_key::<&str>()? {
                            match key {
                                #(#entries)*
                            }
                        }

                        #(let #req_fields = if let ::std::option::Option::Some(val) = #req_fields {
                            val
                        } else {
                            return ::std::result::Result::Err(
                                <M::Error as #zv::export::serde::de::Error>::missing_field(
                                    ::std::stringify!(#req_fields),
                                ),
                            );
                        };)*

                        ::std::result::Result::Ok(#name { #(#fields),* })
                    }
                }


                deserializer.deserialize_map(#visitor(::std::marker::PhantomData))
            }
        }
    })
}
