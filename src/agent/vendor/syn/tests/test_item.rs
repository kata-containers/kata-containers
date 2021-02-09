#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;
use std::iter::FromIterator;
use syn::Item;

#[test]
fn test_macro_variable_attr() {
    // mimics the token stream corresponding to `$attr fn f() {}`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Group(Group::new(Delimiter::None, quote! { #[test] })),
        TokenTree::Ident(Ident::new("fn", Span::call_site())),
        TokenTree::Ident(Ident::new("f", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r###"
    Item::Fn {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "test",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(``),
            },
        ],
        vis: Inherited,
        sig: Signature {
            ident: "f",
            generics: Generics,
            output: Default,
        },
        block: Block,
    }
    "###);
}

#[test]
fn test_negative_impl() {
    // Rustc parses all of the following.

    #[cfg(any())]
    impl ! {}
    let tokens = quote! {
        impl ! {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Type::Never,
    }
    "###);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !Trait {}
    let tokens = quote! {
        impl !Trait {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Verbatim(`! Trait`),
    }
    "###);

    #[cfg(any())]
    impl !Trait for T {}
    let tokens = quote! {
        impl !Trait for T {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            Some,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
                        arguments: None,
                    },
                ],
            },
        )),
        self_ty: Type::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "T",
                        arguments: None,
                    },
                ],
            },
        },
    }
    "###);

    #[cfg(any())]
    #[rustfmt::skip]
    impl !! {}
    let tokens = quote! {
        impl !! {}
    };
    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        self_ty: Verbatim(`! !`),
    }
    "###);
}

#[test]
fn test_macro_variable_impl() {
    // mimics the token stream corresponding to `impl $trait for $ty {}`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Ident(Ident::new("impl", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Trait))),
        TokenTree::Ident(Ident::new("for", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::None, quote!(Type))),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Item, @r###"
    Item::Impl {
        generics: Generics,
        trait_: Some((
            None,
            Path {
                segments: [
                    PathSegment {
                        ident: "Trait",
                        arguments: None,
                    },
                ],
            },
        )),
        self_ty: Type::Group {
            elem: Type::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "Type",
                            arguments: None,
                        },
                    ],
                },
            },
        },
    }
    "###);
}
