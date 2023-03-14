//! Custom derive support for `zeroize`

#![crate_type = "proc-macro"]
#![forbid(unsafe_code)]
#![warn(rust_2018_idioms, trivial_casts, unused_qualifications)]

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
    Attribute, Lit, Meta, NestedMeta, Result, WherePredicate,
};
use synstructure::{decl_derive, AddBounds, BindStyle, BindingInfo, VariantInfo};

decl_derive!(
    [Zeroize, attributes(zeroize)] =>

    /// Derive the `Zeroize` trait.
    ///
    /// Supports the following attributes:
    ///
    /// On the item level:
    /// - `#[zeroize(drop)]`: *deprecated* use `ZeroizeOnDrop` instead
    /// - `#[zeroize(bound = "T: MyTrait")]`: this replaces any trait bounds
    ///   inferred by zeroize-derive
    ///
    /// On the field level:
    /// - `#[zeroize(skip)]`: skips this field or variant when calling `zeroize()`
    derive_zeroize
);

decl_derive!(
    [ZeroizeOnDrop, attributes(zeroize)] =>

    /// Derive the `ZeroizeOnDrop` trait.
    ///
    /// Supports the following attributes:
    ///
    /// On the field level:
    /// - `#[zeroize(skip)]`: skips this field or variant when calling `zeroize()`
    derive_zeroize_on_drop
);

/// Name of zeroize-related attributes
const ZEROIZE_ATTR: &str = "zeroize";

/// Custom derive for `Zeroize`
fn derive_zeroize(mut s: synstructure::Structure<'_>) -> TokenStream {
    let attributes = ZeroizeAttrs::parse(&s);

    if let Some(bounds) = attributes.bound {
        s.add_bounds(AddBounds::None);

        for bound in bounds.0 {
            s.add_where_predicate(bound);
        }
    }

    // NOTE: These are split into named functions to simplify testing with
    // synstructure's `test_derive!` macro.
    if attributes.drop {
        derive_zeroize_with_drop(s)
    } else {
        derive_zeroize_without_drop(s)
    }
}

/// Custom derive for `ZeroizeOnDrop`
fn derive_zeroize_on_drop(mut s: synstructure::Structure<'_>) -> TokenStream {
    let zeroizers = generate_fields(&mut s, quote! { zeroize_or_on_drop });

    let drop_impl = s.add_bounds(AddBounds::None).gen_impl(quote! {
        gen impl Drop for @Self {
            fn drop(&mut self) {
                use zeroize::__internal::AssertZeroize;
                use zeroize::__internal::AssertZeroizeOnDrop;
                match self {
                    #zeroizers
                }
            }
        }
    });

    let zeroize_on_drop_impl = impl_zeroize_on_drop(&s);

    quote! {
        #drop_impl

        #zeroize_on_drop_impl
    }
}

/// Custom derive attributes for `Zeroize`
#[derive(Default)]
struct ZeroizeAttrs {
    /// Derive a `Drop` impl which calls zeroize on this type
    drop: bool,
    /// Custom bounds as defined by the user
    bound: Option<Bounds>,
}

/// Parsing helper for custom bounds
struct Bounds(Punctuated<WherePredicate, Comma>);

impl Parse for Bounds {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

impl ZeroizeAttrs {
    /// Parse attributes from the incoming AST
    fn parse(s: &synstructure::Structure<'_>) -> Self {
        let mut result = Self::default();

        for attr in s.ast().attrs.iter() {
            result.parse_attr(attr, None, None);
        }
        for v in s.variants().iter() {
            // only process actual enum variants here, as we don't want to process struct attributes twice
            if v.prefix.is_some() {
                for attr in v.ast().attrs.iter() {
                    result.parse_attr(attr, Some(v), None);
                }
            }
            for binding in v.bindings().iter() {
                for attr in binding.ast().attrs.iter() {
                    result.parse_attr(attr, Some(v), Some(binding));
                }
            }
        }

        result
    }

    /// Parse attribute and handle `#[zeroize(...)]` attributes
    fn parse_attr(
        &mut self,
        attr: &Attribute,
        variant: Option<&VariantInfo<'_>>,
        binding: Option<&BindingInfo<'_>>,
    ) {
        let meta_list = match attr
            .parse_meta()
            .unwrap_or_else(|e| panic!("error parsing attribute: {:?} ({})", attr, e))
        {
            Meta::List(list) => list,
            _ => return,
        };

        // Ignore any non-zeroize attributes
        if !meta_list.path.is_ident(ZEROIZE_ATTR) {
            return;
        }

        for nested_meta in &meta_list.nested {
            if let NestedMeta::Meta(meta) = nested_meta {
                self.parse_meta(meta, variant, binding);
            } else {
                panic!("malformed #[zeroize] attribute: {:?}", nested_meta);
            }
        }
    }

    /// Parse `#[zeroize(...)]` attribute metadata (e.g. `drop`)
    fn parse_meta(
        &mut self,
        meta: &Meta,
        variant: Option<&VariantInfo<'_>>,
        binding: Option<&BindingInfo<'_>>,
    ) {
        if meta.path().is_ident("drop") {
            assert!(!self.drop, "duplicate #[zeroize] drop flags");

            match (variant, binding) {
                (_variant, Some(_binding)) => {
                    // structs don't have a variant prefix, and only structs have bindings outside of a variant
                    let item_kind = match variant.and_then(|variant| variant.prefix) {
                        Some(_) => "enum",
                        None => "struct",
                    };
                    panic!(
                        concat!(
                            "The #[zeroize(drop)] attribute is not allowed on {} fields. ",
                            "Use it on the containing {} instead.",
                        ),
                        item_kind, item_kind,
                    )
                }
                (Some(_variant), None) => panic!(concat!(
                    "The #[zeroize(drop)] attribute is not allowed on enum variants. ",
                    "Use it on the containing enum instead.",
                )),
                (None, None) => (),
            };

            self.drop = true;
        } else if meta.path().is_ident("bound") {
            assert!(self.bound.is_none(), "duplicate #[zeroize] bound flags");

            match (variant, binding) {
                (_variant, Some(_binding)) => {
                    // structs don't have a variant prefix, and only structs have bindings outside of a variant
                    let item_kind = match variant.and_then(|variant| variant.prefix) {
                        Some(_) => "enum",
                        None => "struct",
                    };
                    panic!(
                        concat!(
                            "The #[zeroize(bound)] attribute is not allowed on {} fields. ",
                            "Use it on the containing {} instead.",
                        ),
                        item_kind, item_kind,
                    )
                }
                (Some(_variant), None) => panic!(concat!(
                    "The #[zeroize(bound)] attribute is not allowed on enum variants. ",
                    "Use it on the containing enum instead.",
                )),
                (None, None) => {
                    if let Meta::NameValue(meta_name_value) = meta {
                        if let Lit::Str(lit) = &meta_name_value.lit {
                            if lit.value().is_empty() {
                                self.bound = Some(Bounds(Punctuated::new()));
                            } else {
                                self.bound = Some(lit.parse().unwrap_or_else(|e| {
                                    panic!("error parsing bounds: {:?} ({})", lit, e)
                                }));
                            }

                            return;
                        }
                    }

                    panic!(concat!(
                        "The #[zeroize(bound)] attribute expects a name-value syntax with a string literal value.",
                        "E.g. #[zeroize(bound = \"T: MyTrait\")]."
                    ))
                }
            }
        } else if meta.path().is_ident("skip") {
            if variant.is_none() && binding.is_none() {
                panic!(concat!(
                    "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. ",
                    "Use it on a field or variant instead.",
                ))
            }
        } else {
            panic!("unknown #[zeroize] attribute type: {:?}", meta.path());
        }
    }
}

fn generate_fields(s: &mut synstructure::Structure<'_>, method: TokenStream) -> TokenStream {
    s.bind_with(|_| BindStyle::RefMut);

    s.filter_variants(|vi| {
        let result = filter_skip(vi.ast().attrs, true);

        // check for duplicate `#[zeroize(skip)]` attributes in nested variants
        for field in vi.ast().fields {
            filter_skip(&field.attrs, result);
        }

        result
    })
    .filter(|bi| filter_skip(&bi.ast().attrs, true))
    .each(|bi| quote! { #bi.#method(); })
}

fn filter_skip(attrs: &[Attribute], start: bool) -> bool {
    let mut result = start;

    for attr in attrs.iter().filter_map(|attr| attr.parse_meta().ok()) {
        if let Meta::List(list) = attr {
            if list.path.is_ident(ZEROIZE_ATTR) {
                for nested in list.nested {
                    if let NestedMeta::Meta(Meta::Path(path)) = nested {
                        if path.is_ident("skip") {
                            assert!(result, "duplicate #[zeroize] skip flags");
                            result = false;
                        }
                    }
                }
            }
        }
    }

    result
}

/// Custom derive for `Zeroize` (without `Drop`)
fn derive_zeroize_without_drop(mut s: synstructure::Structure<'_>) -> TokenStream {
    let zeroizers = generate_fields(&mut s, quote! { zeroize });

    s.bound_impl(
        quote!(zeroize::Zeroize),
        quote! {
            fn zeroize(&mut self) {
                match self {
                    #zeroizers
                }
            }
        },
    )
}

/// Custom derive for `Zeroize` and `Drop`
fn derive_zeroize_with_drop(s: synstructure::Structure<'_>) -> TokenStream {
    let drop_impl = s.gen_impl(quote! {
        gen impl Drop for @Self {
            fn drop(&mut self) {
                self.zeroize();
            }
        }
    });

    let zeroize_impl = derive_zeroize_without_drop(s);

    quote! {
        #zeroize_impl

        #[doc(hidden)]
        #drop_impl
    }
}

fn impl_zeroize_on_drop(s: &synstructure::Structure<'_>) -> TokenStream {
    #[allow(unused_qualifications)]
    s.bound_impl(quote!(zeroize::ZeroizeOnDrop), Option::<TokenStream>::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;
    use synstructure::{test_derive, Structure};

    #[test]
    fn zeroize_without_drop() {
        test_derive! {
            derive_zeroize_without_drop {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            }
            expands to {
                #[allow(non_upper_case_globals)]
                #[doc(hidden)]
                const _DERIVE_zeroize_Zeroize_FOR_Z: () = {
                    extern crate zeroize;
                    impl zeroize::Zeroize for Z {
                        fn zeroize(&mut self) {
                            match self {
                                Z {
                                    a: ref mut __binding_0,
                                    b: ref mut __binding_1,
                                    c: ref mut __binding_2,
                                } => {
                                    { __binding_0.zeroize(); }
                                    { __binding_1.zeroize(); }
                                    { __binding_2.zeroize(); }
                                }
                            }
                        }
                    }
                };
            }
            no_build // tests the code compiles are in the `zeroize` crate
        }
    }

    #[test]
    fn zeroize_with_drop() {
        test_derive! {
            derive_zeroize_with_drop {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            }
            expands to {
                #[allow(non_upper_case_globals)]
                #[doc(hidden)]
                const _DERIVE_zeroize_Zeroize_FOR_Z: () = {
                    extern crate zeroize;
                    impl zeroize::Zeroize for Z {
                        fn zeroize(&mut self) {
                            match self {
                                Z {
                                    a: ref mut __binding_0,
                                    b: ref mut __binding_1,
                                    c: ref mut __binding_2,
                                } => {
                                    { __binding_0.zeroize(); }
                                    { __binding_1.zeroize(); }
                                    { __binding_2.zeroize(); }
                                }
                            }
                        }
                    }
                };
                #[doc(hidden)]
                #[allow(non_upper_case_globals)]
                const _DERIVE_Drop_FOR_Z: () = {
                    impl Drop for Z {
                        fn drop(&mut self) {
                            self.zeroize();
                        }
                    }
                };
            }
            no_build // tests the code compiles are in the `zeroize` crate
        }
    }

    #[test]
    fn zeroize_with_skip() {
        test_derive! {
            derive_zeroize_without_drop {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    #[zeroize(skip)]
                    c: [u8; 3],
                }
            }
            expands to {
                #[allow(non_upper_case_globals)]
                #[doc(hidden)]
                const _DERIVE_zeroize_Zeroize_FOR_Z: () = {
                    extern crate zeroize;
                    impl zeroize::Zeroize for Z {
                        fn zeroize(&mut self) {
                            match self {
                                Z {
                                    a: ref mut __binding_0,
                                    b: ref mut __binding_1,
                                    ..
                                } => {
                                    { __binding_0.zeroize(); }
                                    { __binding_1.zeroize(); }
                                }
                            }
                        }
                    }
                };
            }
            no_build // tests the code compiles are in the `zeroize` crate
        }
    }

    #[test]
    fn zeroize_with_bound() {
        test_derive! {
            derive_zeroize {
                #[zeroize(bound = "T: MyTrait")]
                struct Z<T>(T);
            }
            expands to {
                #[allow(non_upper_case_globals)]
                #[doc(hidden)]
                const _DERIVE_zeroize_Zeroize_FOR_Z: () = {
                    extern crate zeroize;
                    impl<T> zeroize::Zeroize for Z<T>
                    where T: MyTrait
                    {
                        fn zeroize(&mut self) {
                            match self {
                                Z(ref mut __binding_0,) => {
                                    { __binding_0.zeroize(); }
                                }
                            }
                        }
                    }
                };
            }
            no_build // tests the code compiles are in the `zeroize` crate
        }
    }

    #[test]
    fn zeroize_only_drop() {
        test_derive! {
            derive_zeroize_on_drop {
                struct Z {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                }
            }
            expands to {
                #[allow(non_upper_case_globals)]
                const _DERIVE_Drop_FOR_Z: () = {
                    impl Drop for Z {
                        fn drop(&mut self) {
                            use zeroize::__internal::AssertZeroize;
                            use zeroize::__internal::AssertZeroizeOnDrop;
                            match self {
                                Z {
                                    a: ref mut __binding_0,
                                    b: ref mut __binding_1,
                                    c: ref mut __binding_2,
                                } => {
                                    { __binding_0.zeroize_or_on_drop(); }
                                    { __binding_1.zeroize_or_on_drop(); }
                                    { __binding_2.zeroize_or_on_drop(); }
                                }
                            }
                        }
                    }
                };
                #[allow(non_upper_case_globals)]
                #[doc(hidden)]
                const _DERIVE_zeroize_ZeroizeOnDrop_FOR_Z: () = {
                    extern crate zeroize;
                    impl zeroize::ZeroizeOnDrop for Z {}
                };
            }
            no_build // tests the code compiles are in the `zeroize` crate
        }
    }

    #[test]
    fn zeroize_on_struct() {
        parse_zeroize_test(stringify!(
            #[zeroize(drop)]
            struct Z {
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    fn zeroize_on_enum() {
        parse_zeroize_test(stringify!(
            #[zeroize(drop)]
            enum Z {
                Variant1 { a: String, b: Vec<u8>, c: [u8; 3] },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_struct_field() {
        parse_zeroize_test(stringify!(
            struct Z {
                #[zeroize(drop)]
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_tuple_struct_field() {
        parse_zeroize_test(stringify!(
            struct Z(#[zeroize(drop)] String);
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on struct fields")]
    fn zeroize_on_second_field() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(drop)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_tuple_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant(#[zeroize(drop)] String),
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant {
                    #[zeroize(drop)]
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum fields")]
    fn zeroize_on_enum_second_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant1 {
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
                Variant2 {
                    #[zeroize(drop)]
                    a: String,
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum variants")]
    fn zeroize_on_enum_variant() {
        parse_zeroize_test(stringify!(
            enum Z {
                #[zeroize(drop)]
                Variant,
            }
        ));
    }

    #[test]
    #[should_panic(expected = "#[zeroize(drop)] attribute is not allowed on enum variants")]
    fn zeroize_on_enum_second_variant() {
        parse_zeroize_test(stringify!(
            enum Z {
                Variant1,
                #[zeroize(drop)]
                Variant2,
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. Use it on a field or variant instead."
    )]
    fn zeroize_skip_on_struct() {
        parse_zeroize_test(stringify!(
            #[zeroize(skip)]
            struct Z {
                a: String,
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(skip)] attribute is not allowed on a `struct` or `enum`. Use it on a field or variant instead."
    )]
    fn zeroize_skip_on_enum() {
        parse_zeroize_test(stringify!(
            #[zeroize(skip)]
            enum Z {
                Variant1,
                Variant2,
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(skip)]
                #[zeroize(skip)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip_list() {
        parse_zeroize_test(stringify!(
            struct Z {
                a: String,
                #[zeroize(skip, skip)]
                b: Vec<u8>,
                c: [u8; 3],
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] skip flags")]
    fn zeroize_duplicate_skip_enum() {
        parse_zeroize_test(stringify!(
            enum Z {
                #[zeroize(skip)]
                Variant {
                    a: String,
                    #[zeroize(skip)]
                    b: Vec<u8>,
                    c: [u8; 3],
                },
            }
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] bound flags")]
    fn zeroize_duplicate_bound() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T: MyTrait")]
            #[zeroize(bound = "")]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(expected = "duplicate #[zeroize] bound flags")]
    fn zeroize_duplicate_bound_list() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T: MyTrait", bound = "")]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on struct fields. Use it on the containing struct instead."
    )]
    fn zeroize_bound_struct() {
        parse_zeroize_test(stringify!(
            struct Z<T> {
                #[zeroize(bound = "T: MyTrait")]
                a: T,
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on enum variants. Use it on the containing enum instead."
    )]
    fn zeroize_bound_enum() {
        parse_zeroize_test(stringify!(
            enum Z<T> {
                #[zeroize(bound = "T: MyTrait")]
                A(T),
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute is not allowed on enum fields. Use it on the containing enum instead."
    )]
    fn zeroize_bound_enum_variant_field() {
        parse_zeroize_test(stringify!(
            enum Z<T> {
                A {
                    #[zeroize(bound = "T: MyTrait")]
                    a: T,
                },
            }
        ));
    }

    #[test]
    #[should_panic(
        expected = "The #[zeroize(bound)] attribute expects a name-value syntax with a string literal value.E.g. #[zeroize(bound = \"T: MyTrait\")]."
    )]
    fn zeroize_bound_no_value() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound)]
            struct Z<T>(T);
        ));
    }

    #[test]
    #[should_panic(expected = "error parsing bounds: LitStr { token: \"T\" } (expected `:`)")]
    fn zeroize_bound_no_where_predicate() {
        parse_zeroize_test(stringify!(
            #[zeroize(bound = "T")]
            struct Z<T>(T);
        ));
    }

    fn parse_zeroize_test(unparsed: &str) -> TokenStream {
        derive_zeroize(Structure::new(
            &parse_str(unparsed).expect("Failed to parse test input"),
        ))
    }
}
