//! In case of bad input, parsing should fail. The error should have locations set in derived implementations.
#[macro_use]
extern crate darling;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

use darling::FromDeriveInput;

#[derive(Debug, FromMeta)]
struct Dolor {
    #[darling(rename = "amet")]
    sit: bool,
    world: bool,
}

#[derive(Debug, FromDeriveInput)]
#[darling(from_ident, attributes(hello))]
struct Lorem {
    ident: syn::Ident,
    ipsum: Dolor,
}

impl From<syn::Ident> for Lorem {
    fn from(ident: syn::Ident) -> Self {
        Lorem {
            ident,
            ipsum: Dolor {
                sit: false,
                world: true,
            },
        }
    }
}

#[test]
fn parsing_fail() {
    let di = parse_quote! {
        #[hello(ipsum(amet = "yes", world = false))]
        pub struct Foo;
    };

    println!("{}", Lorem::from_derive_input(&di).unwrap_err());
}

#[test]
fn missing_field() {
    let di = parse_quote! {
        #[hello(ipsum(amet = true))]
        pub struct Foo;
    };

    println!("{}", Lorem::from_derive_input(&di).unwrap_err());
}
