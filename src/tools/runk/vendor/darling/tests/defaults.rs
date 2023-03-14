#[macro_use]
extern crate darling;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use darling::FromDeriveInput;

mod foo {
    pub mod bar {
        pub fn init() -> String {
            String::from("hello")
        }
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(speak))]
pub struct SpeakerOpts {
    #[darling(default = "foo::bar::init")]
    first_word: String,
}

#[test]
fn path_default() {
    let speaker: SpeakerOpts = FromDeriveInput::from_derive_input(&parse_quote! {
        struct Foo;
    })
    .expect("Unit struct with no attrs should parse");

    assert_eq!(speaker.first_word, "hello");
}
