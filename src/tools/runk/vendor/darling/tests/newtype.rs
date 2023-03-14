//! A newtype struct should be able to derive `FromMeta` if its member implements it.

#[macro_use]
extern crate darling;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

use darling::FromDeriveInput;

#[derive(Debug, FromMeta, PartialEq, Eq)]
struct Lorem(bool);

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(newtype))]
struct DemoContainer {
    lorem: Lorem,
}

#[test]
fn generated() {
    let di = parse_quote! {
        #[derive(Baz)]
        #[newtype(lorem = false)]
        pub struct Foo;
    };

    let c = DemoContainer::from_derive_input(&di).unwrap();

    assert_eq!(c.lorem, Lorem(false));
}
