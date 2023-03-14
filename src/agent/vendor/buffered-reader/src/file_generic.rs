use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::*;
use crate::file_error::FileError;

/// Wraps files.
///
/// This is a generic implementation that may be replaced by
/// platform-specific versions.
pub struct File<C: fmt::Debug + Sync + Send>(Generic<fs::File, C>, PathBuf);

assert_send_and_sync!(File<C>
                      where C: fmt::Debug);

impl<C: fmt::Debug + Sync + Send> fmt::Display for File<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "File {:?}", self.1.display())
    }
}

impl<C: fmt::Debug + Sync + Send> fmt::Debug for File<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("File")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl File<()> {
    /// Opens the given file.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::with_cookie(path, ())
    }
}

impl<C: fmt::Debug + Sync + Send> File<C> {
    /// Like `open()`, but sets a cookie.
    pub fn with_cookie<P: AsRef<Path>>(path: P, cookie: C) -> io::Result<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path).map_err(|e| FileError::new(path, e))?;
        Ok(File(Generic::with_cookie(file, None, cookie), path.into()))
    }
}

impl<C: fmt::Debug + Sync + Send> io::Read for File<C> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
            .map_err(|e| FileError::new(&self.1, e))
    }
}

impl<C: fmt::Debug + Sync + Send> BufferedReader<C> for File<C> {
    fn buffer(&self) -> &[u8] {
        self.0.buffer()
    }

    fn data(&mut self, amount: usize) -> io::Result<&[u8]> {
        let path = &self.1;
        self.0.data(amount)
            .map_err(|e| FileError::new(path, e))
    }

    fn data_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        let path = &self.1;
        self.0.data_hard(amount)
            .map_err(|e| FileError::new(path, e))
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        self.0.consume(amount)
    }

    fn data_consume(&mut self, amount: usize) -> io::Result<&[u8]> {
        let path = &self.1;
        self.0.data_consume(amount)
            .map_err(|e| FileError::new(path, e))
    }

    fn data_consume_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        let path = &self.1;
        self.0.data_consume_hard(amount)
            .map_err(|e| FileError::new(path, e))
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        None
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        None
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'b>>
        where Self: 'b {
        None
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        self.0.cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &C {
        self.0.cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut C {
        self.0.cookie_mut()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn error_contains_path() {
        let p = "/i/do/not/exist";
        let e = File::open(p).unwrap_err();
        assert!(e.to_string().contains(p));
    }
}
