#[macro_use]
extern crate darling;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

use darling::FromDeriveInput;

#[derive(Debug, Clone, FromMeta)]
struct Wrapper<T>(pub T);

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(hello))]
struct Foo<T> {
    lorem: Wrapper<T>,
}

#[test]
fn expansion() {
    let di = parse_quote! {
        #[hello(lorem = "Hello")]
        pub struct Foo;
    };

    Foo::<String>::from_derive_input(&di).unwrap();
}
