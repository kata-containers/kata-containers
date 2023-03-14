use std::error::Error;
use std::fmt;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ErrorKind {
    SysError,
}

#[derive(Debug)]
enum ErrorRepr {
    FromNix(nix::Error),
    WithDescription(ErrorKind, &'static str),
}

#[derive(Debug)]
pub struct PrivDropError {
    repr: ErrorRepr,
}

impl Error for PrivDropError {
    fn cause(&self) -> Option<&dyn Error> {
        match self.repr {
            ErrorRepr::FromNix(ref e) => Some(e as &dyn Error),
            _ => None,
        }
    }
}

impl fmt::Display for PrivDropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self.repr {
            ErrorRepr::FromNix(ref e) => e.fmt(f),
            ErrorRepr::WithDescription(_, description) => description.fmt(f),
        }
    }
}

impl From<nix::Error> for PrivDropError {
    fn from(e: nix::Error) -> PrivDropError {
        PrivDropError {
            repr: ErrorRepr::FromNix(e),
        }
    }
}

impl From<(ErrorKind, &'static str)> for PrivDropError {
    fn from((kind, description): (ErrorKind, &'static str)) -> PrivDropError {
        PrivDropError {
            repr: ErrorRepr::WithDescription(kind, description),
        }
    }
}
