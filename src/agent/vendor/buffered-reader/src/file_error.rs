/// Common error type for file operations.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Common error type for file operations.
#[derive(Debug)]
pub(crate) struct FileError {
    path: PathBuf,
    source: io::Error,
}

#[allow(clippy::new_ret_no_self)]
impl FileError {
    /// Returns a new `io::Error` backed by a `FileError`.
    pub fn new<P: AsRef<Path>>(path: P, source: io::Error) -> io::Error {
        io::Error::new(source.kind(), FileError {
            path: path.as_ref().into(),
            source,
        })
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Reading {:?}: {}", self.path.display(), self.source)
    }
}

impl Error for FileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}
