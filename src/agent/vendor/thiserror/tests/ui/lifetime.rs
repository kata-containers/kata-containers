use thiserror::Error;

#[derive(Error, Debug)]
#[error("error")]
struct Error<'a>(#[from] Inner<'a>);

#[derive(Error, Debug)]
#[error("{0}")]
struct Inner<'a>(&'a str);

fn main() -> Result<(), Error<'static>> {
    Err(Error(Inner("some text")))
}
