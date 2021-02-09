//! Exits with exit code 0 if backtraces are disabled and 1 if they are enabled.
//! Used by tests to make sure backtraces are available when they should be. Should not be used
//! outside of the tests.

#[macro_use]
extern crate error_chain;

error_chain! {
    errors {
        MyError
    }
}

fn main() {
    let err = Error::from(ErrorKind::MyError);
    let has_backtrace = err.backtrace().is_some();
    ::std::process::exit(has_backtrace as i32);
}
