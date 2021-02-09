#[macro_use]
extern crate failure;

use failure::{err_msg, Error, Fail};

#[derive(Debug, Fail)]
#[fail(display = "my wrapping error")]
struct WrappingError(#[fail(cause)] Error);

fn bad_function() -> Result<(), WrappingError> {
    Err(WrappingError(err_msg("this went bad")))
}

fn main() {
    for cause in Fail::iter_causes(&bad_function().unwrap_err()) {
        println!("{}", cause);
    }
}
