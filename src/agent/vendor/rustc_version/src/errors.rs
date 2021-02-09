use std::{self, error, fmt, io, str};
use semver::{self, Identifier};

/// The error type for this crate.
#[derive(Debug)]
pub enum Error {
    /// An error ocurrend when executing the `rustc` command.
    CouldNotExecuteCommand(io::Error),
    /// The output of `rustc -vV` was not valid utf-8.
    Utf8Error(str::Utf8Error),
    /// The output of `rustc -vV` was not in the expected format.
    UnexpectedVersionFormat,
    /// An error ocurred in parsing a `VersionReq`.
    ReqParseError(semver::ReqParseError),
    /// An error ocurred in parsing the semver.
    SemVerError(semver::SemVerError),
    /// The pre-release tag is unknown.
    UnknownPreReleaseTag(Identifier),
}
use Error::*;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::error::Error;
        match *self {
            CouldNotExecuteCommand(ref e) => write!(f, "{}: {}", self.description(), e),
            Utf8Error(_) => write!(f, "{}", self.description()),
            UnexpectedVersionFormat => write!(f, "{}", self.description()),
            ReqParseError(ref e) => write!(f, "{}: {}", self.description(), e),
            SemVerError(ref e) => write!(f, "{}: {}", self.description(), e),
            UnknownPreReleaseTag(ref i) => write!(f, "{}: {}", self.description(), i),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&error::Error> {
        match *self {
            CouldNotExecuteCommand(ref e) => Some(e),
            Utf8Error(ref e) => Some(e),
            UnexpectedVersionFormat => None,
            ReqParseError(ref e) => Some(e),
            SemVerError(ref e) => Some(e),
            UnknownPreReleaseTag(_) => None,
        }
    }

    fn description(&self) -> &str {
        match *self {
            CouldNotExecuteCommand(_) => "could not execute command",
            Utf8Error(_) => "invalid UTF-8 output from `rustc -vV`",
            UnexpectedVersionFormat => "unexpected `rustc -vV` format",
            ReqParseError(_) => "error parsing version requirement",
            SemVerError(_) => "error parsing version",
            UnknownPreReleaseTag(_) => "unknown pre-release tag",
        }
    }
}

macro_rules! impl_from {
    ($($err_ty:ty => $variant:ident),* $(,)*) => {
        $(
            impl From<$err_ty> for Error {
                fn from(e: $err_ty) -> Error {
                    Error::$variant(e)
                }
            }
        )*
    }
}

impl_from! {
    str::Utf8Error => Utf8Error,
    semver::SemVerError => SemVerError,
    semver::ReqParseError => ReqParseError,
}

/// The result type for this crate.
pub type Result<T> = std::result::Result<T, Error>;
