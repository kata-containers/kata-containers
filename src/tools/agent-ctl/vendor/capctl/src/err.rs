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

    fn strerror(&self) -> &'static str {
        // If the given error number is invalid (negative, 0, or out of range), musl's strerror()
        // returns "No error information".
        //
        // However, for negative or out of range numbers, glibc's strerror() allocates memory and
        // prints "Unknown error %d". This means it can't be 'static.
        //
        // So on glibc, if the error code is negative (or if the message returned by strerror()
        // starts with "Unknown error"), we return "Unknown error" instead.

        #[cfg(any(target_env = "", target_env = "gnu"))]
        static UNKNOWN_ERROR: &str = "Unknown error";
        #[cfg(any(target_env = "", target_env = "gnu"))]
        if self.0 < 0 {
            return UNKNOWN_ERROR;
        }

        let ptr = unsafe { libc::strerror(self.0) };

        debug_assert!(!ptr.is_null());

        #[cfg(feature = "std")]
        let msg = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str().unwrap();

        #[cfg(not(feature = "std"))]
        let msg = {
            let mut len = 0;
            while unsafe { *ptr.add(len) } != 0 {
                len += 1;
            }

            core::str::from_utf8(unsafe { core::slice::from_raw_parts(ptr as *const u8, len) })
                .unwrap()
        };

        #[cfg(any(target_env = "", target_env = "gnu"))]
        if msg.starts_with(UNKNOWN_ERROR) {
            return UNKNOWN_ERROR;
        }

        msg
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.strerror())?;
        write!(f, " (code {})", self.0)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("code", &self.0)
            .field("message", &self.strerror())
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
        assert_eq!(Error::from_code(libc::EISDIR).strerror(), "Is a directory");

        #[cfg(any(target_env = "", target_env = "gnu"))]
        assert_eq!(Error::from_code(-1).strerror(), "Unknown error");

        #[cfg(any(target_env = "", target_env = "gnu"))]
        assert_eq!(Error::from_code(8192).strerror(), "Unknown error");
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
