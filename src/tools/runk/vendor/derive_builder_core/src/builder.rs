use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens, TokenStreamExt};
use syn::punctuated::Punctuated;
use syn::{self, Path, TraitBound, TraitBoundModifier, TypeParamBound};

use doc_comment_from;
use BuildMethod;
use BuilderField;
use BuilderPattern;
use DeprecationNotes;
use Setter;

/// Builder, implementing `quote::ToTokens`.
///
/// # Examples
///
/// Will expand to something like the following (depending on settings):
///
/// ```rust,ignore
/// # extern crate proc_macro2;
/// # #[macro_use]
/// # extern crate quote;
/// # extern crate syn;
/// # #[macro_use]
/// # extern crate derive_builder_core;
/// # use quote::TokenStreamExt;
/// # use derive_builder_core::{Builder, DeprecationNotes};
/// # fn main() {
/// #    let builder = default_builder!();
/// #
/// #    assert_eq!(
/// #       quote!(#builder).to_string(),
/// #       {
/// #           let mut result = quote!();
/// #           #[cfg(not(feature = "clippy"))]
/// #           result.append_all(quote!(#[allow(clippy::all)]));
/// #
/// #           result.append_all(quote!(
/// #[derive(Clone)]
/// pub struct FooBuilder {
///     foo: u32,
/// }
///
/// #[doc="Error type for FooBuilder"]
/// #[derive(Debug)]
/// #[non_exhaustive]
/// pub enum FooBuilderError {
///     /// Uninitialized field
///     UninitializedField(&'static str),
///     /// Custom validation error
///     ValidationError(::derive_builder::export::core::string::String),
/// }
///
/// impl ::derive_builder::export::core::convert::From<&'static str> for FooBuilderError {
///     fn from(s: &'static str) -> Self {
///         Self::UninitializedField(s)
///     }
/// }
///
/// impl ::derive_builder::export::core::convert::From<::derive_builder::export::core::string::String> for FooBuilderError {
///     fn from(s: ::derive_builder::export::core::string::String) -> Self {
///         Self::ValidationError(s)
///     }
/// }
///
/// impl ::derive_builder::export::core::fmt::Display for FooBuilderError {
///     fn fmt(&self, f: &mut ::derive_builder::export::core::fmt::Formatter) -> ::derive_builder::export::core::fmt::Result {
///         match self {
///             Self::UninitializedField(ref field) => write!(f, "`{}` must be initialized", field),
///             Self::ValidationError(ref error) => write!(f, "{}", error),
///         }
///     }
/// }
///
/// #[cfg(not(no_std))]
/// impl std::error::Error for FooBuilderError {}
/// #           ));
/// #           #[cfg(not(feature = "clippy"))]
/// #           result.append_all(quote!(#[allow(clippy::all)]));
/// #
/// #           result.append_all(quote!(
///
/// #[allow(dead_code)]
/// impl FooBuilder {
///     fn bar () -> {
///         unimplemented!()
///     }
/// }
///
/// impl ::derive_builder::export::core::default::Default for FooBuilder {
///     fn default() -> Self {
///         Self {
///            foo: ::derive_builder::export::core::default::Default::default(),
///         }
///     }
/// }
///
/// #           ));
/// #           result
/// #       }.to_string()
/// #   );
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<'a> {
    /// Enables code generation for this builder struct.
    pub enabled: bool,
    /// Name of this builder struct.
    pub ident: syn::Ident,
    /// Pattern of this builder struct.
    pub pattern: BuilderPattern,
    /// Traits to automatically derive on the builder type.
    pub derives: &'a [Path],
    /// Type parameters and lifetimes attached to this builder's struct
    /// definition.
    pub generics: Option<&'a syn::Generics>,
    /// Visibility of the builder struct, e.g. `syn::Visibility::Public`.
    pub visibility: syn::Visibility,
    /// Fields of the builder struct, e.g. `foo: u32,`
    ///
    /// Expects each entry to be terminated by a comma.
    pub fields: Vec<TokenStream>,
    /// Builder field initializers, e.g. `foo: Default::default(),`
    ///
    /// Expects each entry to be terminated by a comma.
    pub field_initializers: Vec<TokenStream>,
    /// Functions of the builder struct, e.g. `fn bar() -> { unimplemented!() }`
    pub functions: Vec<TokenStream>,
    /// Whether or not a generated error type is required.
    ///
    /// This would be `false` in the case where an already-existing error is to be used.
    pub generate_error: bool,
    /// Whether this builder must derive `Clone`.
    ///
    /// This is true even for a builder using the `owned` pattern if there is a field whose setter
    /// uses a different pattern.
    pub must_derive_clone: bool,
    /// Doc-comment of the builder struct.
    pub doc_comment: Option<syn::Attribute>,
    /// Emit deprecation notes to the user.
    pub deprecation_notes: DeprecationNotes,
    /// Whether or not a libstd is used.
    pub std: bool,
}

impl<'a> ToTokens for Builder<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.enabled {
            let builder_vis = &self.visibility;
            let builder_ident = &self.ident;
            let bounded_generics = self.compute_impl_bounds();
            let (impl_generics, _, _) = bounded_generics.split_for_impl();
            let (struct_generics, ty_generics, where_clause) = self
                .generics
                .map(syn::Generics::split_for_impl)
                .map(|(i, t, w)| (Some(i), Some(t), Some(w)))
                .unwrap_or((None, None, None));
            let builder_fields = &self.fields;
            let builder_field_initializers = &self.field_initializers;
            let functions = &self.functions;

            // Create the comma-separated set of derived traits for the builder
            let derive_attr = {
                let clone_trait: Path = parse_quote!(Clone);

                let mut traits: Punctuated<&Path, Token![,]> = Default::default();
                if self.must_derive_clone {
                    traits.push(&clone_trait);
                }
                traits.extend(self.derives);

                if traits.is_empty() {
                    quote!()
                } else {
                    quote!(#[derive(#traits)])
                }
            };

            let builder_doc_comment = &self.doc_comment;
            let deprecation_notes = &self.deprecation_notes.as_item();

            #[cfg(not(feature = "clippy"))]
            tokens.append_all(quote!(#[allow(clippy::all)]));

            tokens.append_all(quote!(
                #derive_attr
                #builder_doc_comment
                #builder_vis struct #builder_ident #struct_generics #where_clause {
                    #(#builder_fields)*
                }
            ));

            #[cfg(not(feature = "clippy"))]
            tokens.append_all(quote!(#[allow(clippy::all)]));

            tokens.append_all(quote!(
                #[allow(dead_code)]
                impl #impl_generics #builder_ident #ty_generics #where_clause {
                    #(#functions)*
                    #deprecation_notes
                }

                impl #impl_generics ::derive_builder::export::core::default::Default for #builder_ident #ty_generics #where_clause {
                    fn default() -> Self {
                        Self {
                            #(#builder_field_initializers)*
                        }
                    }
                }
            ));

            if self.generate_error {
                let builder_error_ident = format_ident!("{}Error", builder_ident);
                let builder_error_doc = format!("Error type for {}", builder_ident);

                tokens.append_all(quote!(
                    #[doc=#builder_error_doc]
                    #[derive(Debug)]
                    #[non_exhaustive]
                    #builder_vis enum #builder_error_ident {
                        /// Uninitialized field
                        UninitializedField(&'static str),
                        /// Custom validation error
                        ValidationError(::derive_builder::export::core::string::String),
                    }

                    impl ::derive_builder::export::core::convert::From<::derive_builder::UninitializedFieldError> for #builder_error_ident {
                        fn from(s: ::derive_builder::UninitializedFieldError) -> Self {
                            Self::UninitializedField(s.field_name())
                        }
                    }

                    impl ::derive_builder::export::core::convert::From<::derive_builder::export::core::string::String> for #builder_error_ident {
                        fn from(s: ::derive_builder::export::core::string::String) -> Self {
                            Self::ValidationError(s)
                        }
                    }

                    impl ::derive_builder::export::core::fmt::Display for #builder_error_ident {
                        fn fmt(&self, f: &mut ::derive_builder::export::core::fmt::Formatter) -> ::derive_builder::export::core::fmt::Result {
                            match self {
                                Self::UninitializedField(ref field) => write!(f, "`{}` must be initialized", field),
                                Self::ValidationError(ref error) => write!(f, "{}", error),
                            }
                        }
                    }
                ));

                if self.std {
                    tokens.append_all(quote!(
                        impl std::error::Error for #builder_error_ident {}
                    ));
                }
            }
        }
    }
}

impl<'a> Builder<'a> {
    /// Set a doc-comment for this item.
    pub fn doc_comment(&mut self, s: String) -> &mut Self {
        self.doc_comment = Some(doc_comment_from(s));
        self
    }

    /// Add a field to the builder
    pub fn push_field(&mut self, f: BuilderField) -> &mut Self {
        self.fields.push(quote!(#f));
        self.field_initializers.push(f.default_initializer_tokens());
        self
    }

    /// Add a setter function to the builder
    pub fn push_setter_fn(&mut self, f: Setter) -> &mut Self {
        self.functions.push(quote!(#f));
        self
    }

    /// Add final build function to the builder
    pub fn push_build_fn(&mut self, f: BuildMethod) -> &mut Self {
        self.functions.push(quote!(#f));
        self
    }

    /// Add `Clone` trait bound to generic types for non-owned builders.
    /// This enables target types to declare generics without requiring a
    /// `Clone` impl. This is the same as how the built-in derives for
    /// `Clone`, `Default`, `PartialEq`, and other traits work.
    fn compute_impl_bounds(&self) -> syn::Generics {
        if let Some(type_gen) = self.generics {
            let mut generics = type_gen.clone();

            if !self.pattern.requires_clone() || type_gen.type_params().next().is_none() {
                return generics;
            }

            let clone_bound = TypeParamBound::Trait(TraitBound {
                paren_token: None,
                modifier: TraitBoundModifier::None,
                lifetimes: None,
                path: syn::parse_str("::derive_builder::export::core::clone::Clone").unwrap(),
            });

            for typ in generics.type_params_mut() {
                typ.bounds.push(clone_bound.clone());
            }

            generics
        } else {
            Default::default()
        }
    }
}

/// Helper macro for unit tests. This is _only_ public in order to be accessible
/// from doc-tests too.
#[doc(hidden)]
#[macro_export]
macro_rules! default_builder {
    () => {
        Builder {
            enabled: true,
            ident: syn::Ident::new("FooBuilder", ::proc_macro2::Span::call_site()),
            pattern: Default::default(),
            derives: &vec![],
            generics: None,
            visibility: syn::parse_str("pub").unwrap(),
            fields: vec![quote!(foo: u32,)],
            field_initializers: vec![quote!(foo: ::derive_builder::export::core::default::Default::default(), )],
            functions: vec![quote!(fn bar() -> { unimplemented!() })],
            generate_error: true,
            must_derive_clone: true,
            doc_comment: None,
            deprecation_notes: DeprecationNotes::default(),
            std: true,
        }
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use proc_macro2::TokenStream;

    fn add_generated_error(result: &mut TokenStream) {
        result.append_all(quote!(
            #[doc="Error type for FooBuilder"]
            #[derive(Debug)]
            #[non_exhaustive]
            pub enum FooBuilderError {
                /// Uninitialized field
                UninitializedField(&'static str),
                /// Custom validation error
                ValidationError(::derive_builder::export::core::string::String),
            }

            impl ::derive_builder::export::core::convert::From<::derive_builder::UninitializedFieldError> for FooBuilderError {
                fn from(s: ::derive_builder::UninitializedFieldError) -> Self {
                    Self::UninitializedField(s.field_name())
                }
            }

            impl ::derive_builder::export::core::convert::From<::derive_builder::export::core::string::String> for FooBuilderError {
                fn from(s: ::derive_builder::export::core::string::String) -> Self {
                    Self::ValidationError(s)
                }
            }

            impl ::derive_builder::export::core::fmt::Display for FooBuilderError {
                fn fmt(&self, f: &mut ::derive_builder::export::core::fmt::Formatter) -> ::derive_builder::export::core::fmt::Result {
                    match self {
                        Self::UninitializedField(ref field) => write!(f, "`{}` must be initialized", field),
                        Self::ValidationError(ref error) => write!(f, "{}", error),
                    }
                }
            }

            impl std::error::Error for FooBuilderError {}
        ));
    }

    #[test]
    fn simple() {
        let builder = default_builder!();

        assert_eq!(
            quote!(#builder).to_string(),
            {
                let mut result = quote!();

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[derive(Clone)]
                    pub struct FooBuilder {
                        foo: u32,
                    }
                ));

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[allow(dead_code)]
                    impl FooBuilder {
                        fn bar () -> {
                            unimplemented!()
                        }
                    }

                    impl ::derive_builder::export::core::default::Default for FooBuilder {
                        fn default() -> Self {
                            Self {
                                foo: ::derive_builder::export::core::default::Default::default(),
                            }
                        }
                    }
                ));

                add_generated_error(&mut result);

                result
            }
            .to_string()
        );
    }

    // This test depends on the exact formatting of the `stringify`'d code,
    // so we don't automatically format the test
    #[rustfmt::skip]
    #[test]
    fn generic() {
        let ast: syn::DeriveInput = syn::parse_str(stringify!(
            struct Lorem<'a, T: Debug> where T: PartialEq { }
        )).expect("Couldn't parse item");
        let generics = ast.generics;
        let mut builder = default_builder!();
        builder.generics = Some(&generics);

        assert_eq!(
            quote!(#builder).to_string(),
            {
                let mut result = quote!();

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[derive(Clone)]
                    pub struct FooBuilder<'a, T: Debug> where T: PartialEq {
                        foo: u32,
                    }
                ));

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[allow(dead_code)]
                    impl<'a, T: Debug + ::derive_builder::export::core::clone::Clone> FooBuilder<'a, T> where T: PartialEq {
                        fn bar() -> {
                            unimplemented!()
                        }
                    }

                    impl<'a, T: Debug + ::derive_builder::export::core::clone::Clone> ::derive_builder::export::core::default::Default for FooBuilder<'a, T> where T: PartialEq {
                        fn default() -> Self {
                            Self {
                                foo: ::derive_builder::export::core::default::Default::default(),
                            }
                        }
                    }
                ));

                add_generated_error(&mut result);

                result
            }.to_string()
        );
    }

    // This test depends on the exact formatting of the `stringify`'d code,
    // so we don't automatically format the test
    #[rustfmt::skip]
    #[test]
    fn generic_reference() {
        let ast: syn::DeriveInput = syn::parse_str(stringify!(
            struct Lorem<'a, T: 'a + Default> where T: PartialEq{ }
        )).expect("Couldn't parse item");

        let generics = ast.generics;
        let mut builder = default_builder!();
        builder.generics = Some(&generics);

        assert_eq!(
            quote!(#builder).to_string(),
            {
                let mut result = quote!();

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[derive(Clone)]
                    pub struct FooBuilder<'a, T: 'a + Default> where T: PartialEq {
                        foo: u32,
                    }
                ));

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[allow(dead_code)]
                    impl<'a, T: 'a + Default + ::derive_builder::export::core::clone::Clone> FooBuilder<'a, T>
                    where
                        T: PartialEq
                    {
                        fn bar() -> {
                            unimplemented!()
                        }
                    }

                    impl<'a, T: 'a + Default + ::derive_builder::export::core::clone::Clone> ::derive_builder::export::core::default::Default for FooBuilder<'a, T> where T: PartialEq {
                        fn default() -> Self {
                            Self {
                                foo: ::derive_builder::export::core::default::Default::default(),
                            }
                        }
                    }
                ));

                add_generated_error(&mut result);

                result
            }.to_string()
        );
    }

    // This test depends on the exact formatting of the `stringify`'d code,
    // so we don't automatically format the test
    #[rustfmt::skip]
    #[test]
    fn owned_generic() {
        let ast: syn::DeriveInput = syn::parse_str(stringify!(
            struct Lorem<'a, T: Debug> where T: PartialEq { }
        )).expect("Couldn't parse item");
        let generics = ast.generics;
        let mut builder = default_builder!();
        builder.generics = Some(&generics);
        builder.pattern = BuilderPattern::Owned;
        builder.must_derive_clone = false;

        assert_eq!(
            quote!(#builder).to_string(),
            {
                let mut result = quote!();

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    pub struct FooBuilder<'a, T: Debug> where T: PartialEq {
                        foo: u32,
                    }
                ));

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[allow(dead_code)]
                    impl<'a, T: Debug> FooBuilder<'a, T> where T: PartialEq {
                        fn bar() -> {
                            unimplemented!()
                        }
                    }

                    impl<'a, T: Debug> ::derive_builder::export::core::default::Default for FooBuilder<'a, T>
                    where T: PartialEq {
                        fn default() -> Self {
                            Self {
                                foo: ::derive_builder::export::core::default::Default::default(),
                            }
                        }
                    }
                ));

                add_generated_error(&mut result);

                result
            }.to_string()
        );
    }

    #[test]
    fn disabled() {
        let mut builder = default_builder!();
        builder.enabled = false;

        assert_eq!(quote!(#builder).to_string(), quote!().to_string());
    }

    #[test]
    fn add_derives() {
        let derives = vec![syn::parse_str("Serialize").unwrap()];
        let mut builder = default_builder!();
        builder.derives = &derives;

        assert_eq!(
            quote!(#builder).to_string(),
            {
                let mut result = quote!();

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[derive(Clone, Serialize)]
                    pub struct FooBuilder {
                        foo: u32,
                    }
                ));

                #[cfg(not(feature = "clippy"))]
                result.append_all(quote!(#[allow(clippy::all)]));

                result.append_all(quote!(
                    #[allow(dead_code)]
                    impl FooBuilder {
                        fn bar () -> {
                            unimplemented!()
                        }
                    }

                    impl ::derive_builder::export::core::default::Default for FooBuilder {
                        fn default() -> Self {
                            Self {
                                foo: ::derive_builder::export::core::default::Default::default(),
                            }
                        }
                    }
                ));

                add_generated_error(&mut result);

                result
            }
            .to_string()
        );
    }
}
