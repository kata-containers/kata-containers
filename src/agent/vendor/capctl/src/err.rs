use core::fmt;

pub type Result<T> = core::result::Result<T, Error>;

/// Represents an OS error encountered when performing an operation.
///
/// Note: Parsing errors (i.e. errors returned by `FromStr` implementations) have their own types;
/// for example [`ParseCapError`]).
///
/// [`ParseCapError`]: ./caps/struct.ParseCapError.html
pub struct Error(i32);

impl Error {
    /// Get the last OS error that occured (i.e. the current `errno` value).
    #[inline]
    pub fn last() -> Self {
        Self(unsafe { *libc::__errno_location() })
    }

    /// Construct an `Error` from an `errno` code.
    #[inline]
    pub fn from_code(eno: i32) -> Self {
        Self(eno)
    }

    /// Get the `errno` code represented by this `Error` object.
    #[inline]
    pub fn code(&self) -> i32 {
        self.0
    }

    fn strerror<'a>(&self, buf: &'a mut [u8]) -> &'a str {
        static UNKNOWN_ERROR: &str = "Unknown error";
        if self.0 < 0 {
            return UNKNOWN_ERROR;
        }

        let ret = unsafe { libc::strerror_r(self.0, buf.as_mut_ptr() as *mut _, buf.len()) };
        if ret == libc::EINVAL {
            return UNKNOWN_ERROR;
        }
        assert_eq!(ret, 0, "strerror_r() returned {}", ret);

        #[cfg(feature = "std")]
        let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const _) }
            .to_str()
            .unwrap();

        #[cfg(not(feature = "std"))]
        let msg = {
            let len = buf.iter().position(|&ch| ch == 0).unwrap();
            core::str::from_utf8(&buf[..len]).unwrap()
        };

        #[cfg(target_env = "musl")]
        if msg == "No error information" {
            return UNKNOWN_ERROR;
        }

        msg
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 1024];
        f.write_str(self.strerror(&mut buf))?;
        write!(f, " (code {})", self.0)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 1024];
        let message = self.strerror(&mut buf);
        f.debug_struct("Error")
            .field("code", &self.0)
            .field("message", &message)
            .finish()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    #[inline]
    fn from(e: Error) -> Self {
        Self::from_raw_os_error(e.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code() {
        assert_eq!(Error::from_code(libc::EPERM).code(), libc::EPERM);
        assert_eq!(Error::from_code(libc::ENOENT).code(), libc::ENOENT);
    }

    #[test]
    fn test_last() {
        unsafe {
            *libc::__errno_location() = libc::EPERM;
        }
        assert_eq!(Error::last().code(), libc::EPERM);

        unsafe {
            *libc::__errno_location() = libc::ENOENT;
        }
        assert_eq!(Error::last().code(), libc::ENOENT);
    }

    #[test]
    fn test_strerror() {
        let mut buf = [0u8; 1024];

        assert_eq!(
            Error::from_code(libc::EISDIR).strerror(&mut buf),
            "Is a directory"
        );

        assert_eq!(Error::from_code(-1).strerror(&mut buf), "Unknown error");
        assert_eq!(Error::from_code(8192).strerror(&mut buf), "Unknown error");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_display() {
        assert_eq!(
            Error::from_code(libc::EISDIR).to_string(),
            format!("Is a directory (code {})", libc::EISDIR)
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_debug() {
        assert_eq!(
            format!("{:?}", Error::from_code(libc::EISDIR)),
            format!(
                "Error {{ code: {}, message: \"Is a directory\" }}",
                libc::EISDIR
            )
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_from_error() {
        assert_eq!(
            std::io::Error::from(Error::from_code(libc::ENOENT)).raw_os_error(),
            Some(libc::ENOENT)
        );
    }
}
