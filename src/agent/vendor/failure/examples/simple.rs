#[macro_use]
extern crate failure;

use failure::Fail;

#[derive(Debug, Fail)]
#[fail(display = "my error")]
struct MyError;

#[derive(Debug, Fail)]
#[fail(display = "my wrapping error")]
struct WrappingError(#[fail(cause)] MyError);

fn bad_function() -> Result<(), WrappingError> {
    Err(WrappingError(MyError))
}

fn main() {
    for cause in Fail::iter_chain(&bad_function().unwrap_err()) {
        println!("{}: {}", cause.name().unwrap_or("Error"), cause);
    }
}
