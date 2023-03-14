//! Test expansion of enums which have struct variants.

#[macro_use]
extern crate darling;
extern crate syn;

#[derive(Debug, FromMeta)]
#[darling(rename_all = "snake_case")]
enum Message {
    Hello { user: String, silent: bool },
    Ping,
    Goodbye { user: String },
}

#[test]
fn expansion() {}
