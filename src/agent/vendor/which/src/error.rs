use failure::{Backtrace, Context, Fail};
use std;
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

// To suppress false positives from cargo-clippy
#[cfg_attr(feature = "cargo-clippy", allow(empty_line_after_outer_attr))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ErrorKind {
    BadAbsolutePath,
    BadRelativePath,
    CannotFindBinaryPath,
    CannotGetCurrentDir,
}

impl Fail for ErrorKind {}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let display = match *self {
            ErrorKind::BadAbsolutePath => "Bad absolute path",
            ErrorKind::BadRelativePath => "Bad relative path",
            ErrorKind::CannotFindBinaryPath => "Cannot find binary path",
            ErrorKind::CannotGetCurrentDir => "Cannot get current directory",
        };
        f.write_str(display)
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
