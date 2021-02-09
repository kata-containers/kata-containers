#[macro_use]
extern crate failure;

use failure::Fail;

#[test]
fn test_name() {
    #[derive(Fail, Debug)]
    #[fail(display = "my error")]
    struct MyError;

    let err = MyError;

    assert_eq!(err.to_string(), "my error");
    assert_eq!(err.name(), Some("basic_fail::MyError"));

    let ctx = err.context("whatever");

    assert_eq!(ctx.to_string(), "whatever");
    assert_eq!(ctx.name(), Some("basic_fail::MyError"));
}