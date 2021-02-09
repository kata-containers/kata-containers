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
