use crate::path::Path;
use serde::{de, ser};
use std::error;
use std::fmt::{self, Debug, Display};
use std::io;
use std::result;
use std::str;
use std::string;
use std::sync::Arc;
use yaml_rust::emitter;
use yaml_rust::scanner::{self, Marker, ScanError};

/// An error that happened serializing or deserializing YAML data.
pub struct Error(Box<ErrorImpl>);

/// Alias for a `Result` with the error type `serde_yaml::Error`.
pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum ErrorImpl {
    Message(String, Option<Pos>),

    Emit(emitter::EmitError),
    Scan(scanner::ScanError),
    Io(io::Error),
    Utf8(str::Utf8Error),
    FromUtf8(string::FromUtf8Error),

    EndOfStream,
    MoreThanOneDocument,
    RecursionLimitExceeded,

    Shared(Arc<ErrorImpl>),
}

#[derive(Debug)]
pub struct Pos {
    marker: Marker,
    path: String,
}

/// The input location that an error occured.
#[derive(Debug)]
pub struct Location {
    index: usize,
    line: usize,
    column: usize,
}

impl Location {
    /// The byte index of the error
    pub fn index(&self) -> usize {
        self.index
    }

    /// The line of the error
    pub fn line(&self) -> usize {
        self.line
    }

    /// The column of the error
    pub fn column(&self) -> usize {
        self.column
    }

    // This is to keep decoupled with the yaml crate
    #[doc(hidden)]
    fn from_marker(marker: &Marker) -> Self {
        Location {
            // `col` returned from the `yaml` crate is 0-indexed but all error messages add + 1 to this value
            column: marker.col() + 1,
            index: marker.index(),
            line: marker.line(),
        }
    }
}

impl Error {
    /// Returns the Location from the error if one exists.
    ///
    /// Not all types of errors have a location so this can return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use serde_yaml::{Value, Error};
    /// #
    /// // The `@` character as the first character makes this invalid yaml
    /// let invalid_yaml: Result<Value, Error> = serde_yaml::from_str("@invalid_yaml");
    ///
    /// let location = invalid_yaml.unwrap_err().location().unwrap();
    ///
    /// assert_eq!(location.line(), 1);
    /// assert_eq!(location.column(), 1);
    /// ```
    pub fn location(&self) -> Option<Location> {
        match self.0.as_ref() {
            ErrorImpl::Message(_, Some(pos)) => Some(Location::from_marker(&pos.marker)),
            ErrorImpl::Scan(scan) => Some(Location::from_marker(scan.marker())),
            _ => None,
        }
    }
}

pub(crate) fn end_of_stream() -> Error {
    Error(Box::new(ErrorImpl::EndOfStream))
}

pub(crate) fn more_than_one_document() -> Error {
    Error(Box::new(ErrorImpl::MoreThanOneDocument))
}

pub(crate) fn io(err: io::Error) -> Error {
    Error(Box::new(ErrorImpl::Io(err)))
}

pub(crate) fn emitter(err: emitter::EmitError) -> Error {
    Error(Box::new(ErrorImpl::Emit(err)))
}

pub(crate) fn scanner(err: scanner::ScanError) -> Error {
    Error(Box::new(ErrorImpl::Scan(err)))
}

pub(crate) fn str_utf8(err: str::Utf8Error) -> Error {
    Error(Box::new(ErrorImpl::Utf8(err)))
}

pub(crate) fn string_utf8(err: string::FromUtf8Error) -> Error {
    Error(Box::new(ErrorImpl::FromUtf8(err)))
}

pub(crate) fn recursion_limit_exceeded() -> Error {
    Error(Box::new(ErrorImpl::RecursionLimitExceeded))
}

pub(crate) fn shared(shared: Arc<ErrorImpl>) -> Error {
    Error(Box::new(ErrorImpl::Shared(shared)))
}

pub(crate) fn fix_marker(mut error: Error, marker: Marker, path: Path) -> Error {
    if let ErrorImpl::Message(_, none @ None) = error.0.as_mut() {
        *none = Some(Pos {
            marker,
            path: path.to_string(),
        });
    }
    error
}

impl Error {
    pub(crate) fn shared(self) -> Arc<ErrorImpl> {
        if let ErrorImpl::Shared(err) = *self.0 {
            err
        } else {
            Arc::from(self.0)
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.0.source()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.display(f)
    }
}

// Remove two layers of verbosity from the debug representation. Humans often
// end up seeing this representation because it is what unwrap() shows.
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.debug(f)
    }
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(Box::new(ErrorImpl::Message(msg.to_string(), None)))
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(Box::new(ErrorImpl::Message(msg.to_string(), None)))
    }
}

impl ErrorImpl {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            ErrorImpl::Scan(err) => Some(err),
            ErrorImpl::Io(err) => Some(err),
            ErrorImpl::Utf8(err) => Some(err),
            ErrorImpl::FromUtf8(err) => Some(err),
            ErrorImpl::Shared(err) => err.source(),
            _ => None,
        }
    }

    fn display(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorImpl::Message(msg, None) => Display::fmt(msg, f),
            ErrorImpl::Message(msg, Some(Pos { marker, path })) => {
                if path == "." {
                    write!(f, "{}", ScanError::new(*marker, msg))
                } else {
                    write!(f, "{}: {}", path, ScanError::new(*marker, msg))
                }
            }
            ErrorImpl::Emit(emitter::EmitError::FmtError(_)) => f.write_str("yaml-rust fmt error"),
            ErrorImpl::Emit(emitter::EmitError::BadHashmapKey) => f.write_str("bad hash map key"),
            ErrorImpl::Scan(err) => Display::fmt(err, f),
            ErrorImpl::Io(err) => Display::fmt(err, f),
            ErrorImpl::Utf8(err) => Display::fmt(err, f),
            ErrorImpl::FromUtf8(err) => Display::fmt(err, f),
            ErrorImpl::EndOfStream => f.write_str("EOF while parsing a value"),
            ErrorImpl::MoreThanOneDocument => f.write_str(
                "deserializing from YAML containing more than one document is not supported",
            ),
            ErrorImpl::RecursionLimitExceeded => f.write_str("recursion limit exceeded"),
            ErrorImpl::Shared(err) => err.display(f),
        }
    }

    fn debug(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorImpl::Message(msg, pos) => f.debug_tuple("Message").field(msg).field(pos).finish(),
            ErrorImpl::Emit(emit) => f.debug_tuple("Emit").field(emit).finish(),
            ErrorImpl::Scan(scan) => f.debug_tuple("Scan").field(scan).finish(),
            ErrorImpl::Io(io) => f.debug_tuple("Io").field(io).finish(),
            ErrorImpl::Utf8(utf8) => f.debug_tuple("Utf8").field(utf8).finish(),
            ErrorImpl::FromUtf8(from_utf8) => f.debug_tuple("FromUtf8").field(from_utf8).finish(),
            ErrorImpl::EndOfStream => f.debug_tuple("EndOfStream").finish(),
            ErrorImpl::MoreThanOneDocument => f.debug_tuple("MoreThanOneDocument").finish(),
            ErrorImpl::RecursionLimitExceeded => f.debug_tuple("RecursionLimitExceeded").finish(),
            ErrorImpl::Shared(err) => err.debug(f),
        }
    }
}
