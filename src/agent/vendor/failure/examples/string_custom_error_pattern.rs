//! This example show the pattern "Strings and custom fail type" described in the book
extern crate core;
extern crate failure;

use core::fmt::{self, Display};
use failure::{Backtrace, Context, Fail, ResultExt};

fn main() {
    let err = err1().unwrap_err();
    // Print the error itself
    println!("error: {}", err);
    // Print the chain of errors that caused it
    for cause in Fail::iter_causes(&err) {
        println!("caused by: {}", cause);
    }
}

fn err1() -> Result<(), MyError> {
    // The context can be a String
    Ok(err2().context("err1".to_string())?)
}

fn err2() -> Result<(), MyError> {
    // The context can be a &'static str
    Ok(err3().context("err2")?)
}

fn err3() -> Result<(), MyError> {
    Ok(Err(MyError::from("err3"))?)
}

#[derive(Debug)]
pub struct MyError {
    inner: Context<String>,
}

impl Fail for MyError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

// Allows writing `MyError::from("oops"))?`
impl From<&'static str> for MyError {
    fn from(msg: &'static str) -> MyError {
        MyError {
            inner: Context::new(msg.into()),
        }
    }
}

// Allows adding more context via a String
impl From<Context<String>> for MyError {
    fn from(inner: Context<String>) -> MyError {
        MyError { inner }
    }
}

// Allows adding more context via a &str
impl From<Context<&'static str>> for MyError {
    fn from(inner: Context<&'static str>) -> MyError {
        MyError {
            inner: inner.map(|s| s.to_string()),
        }
    }
}
