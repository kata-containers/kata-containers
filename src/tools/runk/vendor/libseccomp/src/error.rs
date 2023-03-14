// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

pub(crate) type Result<T> = std::result::Result<T, SeccompError>;

// ParseError message
const PARSE_ERROR: &str = "Parse error by invalid argument";

/// Errnos returned by the libseccomp API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
// https://github.com/seccomp/libseccomp/blob/3c0dedd45713d7928c459b6523b78f4cfd435269/src/api.c#L60
pub enum SeccompErrno {
    /// The library doesn't permit the particular operation.
    EACCES,
    /// There was a system failure beyond the control of libseccomp.
    ECANCELED,
    /// Architecture/ABI specific failure.
    EDOM,
    /// Failure regrading the existence of argument.
    EEXIST,
    /// Internal libseccomp failure.
    EFAULT,
    /// Invalid input to the libseccomp API.
    EINVAL,
    /// No matching entry found.
    ENOENT,
    /// Unable to allocate enough memory to perform the requested operation.
    ENOMEM,
    /// The library doesn't support the particular operation.
    EOPNOTSUPP,
    /// Provided buffer is too small.
    ERANGE,
    /// Unable to load the filter due to thread issues.
    ESRCH,
}

impl SeccompErrno {
    fn strerror(&self) -> &'static str {
        use SeccompErrno::*;

        match self {
            EACCES => "The library doesn't permit the particular operation",
            ECANCELED => "There was a system failure beyond the control of libseccomp",
            EDOM => "Architecture/ABI specific failure",
            EEXIST => "Failure regrading the existence of argument",
            EFAULT => "Internal libseccomp failure",
            EINVAL => "Invalid input to the libseccomp API",
            ENOENT => "No matching entry found",
            ENOMEM => "Unable to allocate enough memory to perform the requested operation",
            EOPNOTSUPP => "The library doesn't support the particular operation",
            ERANGE => "Provided buffer is too small",
            ESRCH => "Unable to load the filter due to thread issues",
        }
    }
}

impl fmt::Display for SeccompErrno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.strerror())
    }
}

/// A list specifying different categories of error.
#[derive(Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ErrorKind {
    /// An error that represents error code on failure of the libseccomp API.
    Errno(SeccompErrno),
    /// A parse error occurred while trying to convert a value.
    ParseError,
    /// A lower-level error that is caused by an error from a lower-level module.
    Source,
    /// A custom error that does not fall under any other error kind.
    Common(Cow<'static, str>),
}

/// The error type for libseccomp operations.
pub struct SeccompError {
    kind: ErrorKind,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl SeccompError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    pub(crate) fn with_source<E>(kind: ErrorKind, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn with_msg<M>(msg: M) -> Self
    where
        M: Into<Cow<'static, str>>,
    {
        Self {
            kind: ErrorKind::Common(msg.into()),
            source: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_msg_and_source<M, E>(msg: M, source: E) -> Self
    where
        M: Into<Cow<'static, str>>,
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind: ErrorKind::Common(msg.into()),
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn from_errno(raw_errno: i32) -> Self {
        let seccomp_errno = match -raw_errno {
            libc::EACCES => SeccompErrno::EACCES,
            libc::ECANCELED => SeccompErrno::ECANCELED,
            libc::EDOM => SeccompErrno::EDOM,
            libc::EEXIST => SeccompErrno::EEXIST,
            libc::EFAULT => SeccompErrno::EFAULT,
            libc::EINVAL => SeccompErrno::EINVAL,
            libc::ENOENT => SeccompErrno::ENOENT,
            libc::ENOMEM => SeccompErrno::ENOMEM,
            libc::EOPNOTSUPP => SeccompErrno::EOPNOTSUPP,
            libc::ERANGE => SeccompErrno::ERANGE,
            libc::ESRCH => SeccompErrno::ESRCH,
            _ => {
                return Self::with_msg(format!(
                    "libseccomp-rs error: errno {} not handled.",
                    raw_errno,
                ))
            }
        };
        Self::new(ErrorKind::Errno(seccomp_errno))
    }

    /// Query the errno returned by the libseccomp API.
    pub fn errno(&self) -> Option<SeccompErrno> {
        if let ErrorKind::Errno(errno) = self.kind {
            Some(errno)
        } else {
            None
        }
    }

    fn msg(&self) -> Cow<'_, str> {
        match &self.kind {
            ErrorKind::Errno(e) => e.strerror().into(),
            ErrorKind::Common(s) => s.deref().into(),
            ErrorKind::ParseError => PARSE_ERROR.into(),
            ErrorKind::Source => self.source.as_ref().unwrap().to_string().into(),
        }
    }
}

impl fmt::Display for SeccompError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = self.msg();

        match &self.source {
            Some(source) if self.kind != ErrorKind::Source => {
                write!(f, "{} caused by: {}", msg, source)
            }
            Some(_) | None => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl fmt::Debug for SeccompError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field("message", &self.msg())
            .finish()
    }
}

impl Error for SeccompError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            Some(error) => Some(error.as_ref()),
            None => None,
        }
    }
}

/* Does not work without specialization (RFC 1210) or negative trait bounds
impl<T: Error> From<T> for SeccompError {
    fn from(err: T) -> Self {
        Self::with_source(ErrorKind::Source, err)
    }
}
*/

macro_rules! impl_seccomperror_from {
    ($errty:ty) => {
        impl From<$errty> for SeccompError {
            fn from(err: $errty) -> Self {
                Self::with_source(ErrorKind::Source, err)
            }
        }
    };
}
impl_seccomperror_from!(std::ffi::NulError);
impl_seccomperror_from!(std::num::TryFromIntError);
impl_seccomperror_from!(std::str::Utf8Error);

#[cfg(test)]
mod tests {
    use super::ErrorKind::*;
    use super::*;
    use std::ffi::CString;

    const TEST_ERR_MSG: &str = "test error";
    const TEST_NULL_STR: &str = "f\0oo";
    const NULL_ERR_MSG: &str = "nul byte found in provided data at position: 1";

    #[test]
    fn test_msg() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // Errno
        assert_eq!(
            SeccompError::from_errno(-libc::EACCES).msg(),
            SeccompErrno::EACCES.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ECANCELED).msg(),
            SeccompErrno::ECANCELED.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EDOM).msg(),
            SeccompErrno::EDOM.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EEXIST).msg(),
            SeccompErrno::EEXIST.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EFAULT).msg(),
            SeccompErrno::EFAULT.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EINVAL).msg(),
            SeccompErrno::EINVAL.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ENOENT).msg(),
            SeccompErrno::ENOENT.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ENOMEM).msg(),
            SeccompErrno::ENOMEM.strerror()
        );
        assert_eq!(
            SeccompError::new(Errno(SeccompErrno::EOPNOTSUPP)).msg(),
            SeccompErrno::EOPNOTSUPP.strerror(),
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ERANGE).msg(),
            SeccompErrno::ERANGE.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::ESRCH).msg(),
            SeccompErrno::ESRCH.strerror()
        );
        assert_eq!(
            SeccompError::from_errno(-libc::EPIPE).msg(),
            format!("libseccomp-rs error: errno {} not handled.", -libc::EPIPE)
        );

        // Common
        assert_eq!(
            SeccompError::new(Common(TEST_ERR_MSG.into())).msg(),
            TEST_ERR_MSG
        );

        // ParseError
        assert_eq!(SeccompError::new(ParseError).msg(), PARSE_ERROR);

        // Source
        assert_eq!(
            SeccompError::with_source(Source, null_err).msg(),
            NULL_ERR_MSG
        );
    }

    #[test]
    fn test_source() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        assert!(SeccompError::new(Errno(SeccompErrno::EACCES))
            .source()
            .is_none());
        assert!(
            SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err)
                .source()
                .is_some()
        );
    }

    #[test]
    fn test_from() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();
        let scmp_err = SeccompError::from(null_err.clone());

        assert_eq!(scmp_err.kind, ErrorKind::Source);
        assert_eq!(scmp_err.source().unwrap().to_string(), null_err.to_string());
    }

    #[test]
    fn test_display() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // Errno without source
        assert_eq!(
            format!("{}", SeccompError::new(Errno(SeccompErrno::EACCES))),
            SeccompErrno::EACCES.strerror()
        );
        // Errno with source
        assert_eq!(
            format!(
                "{}",
                SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err.clone())
            ),
            format!(
                "{} caused by: {}",
                SeccompErrno::EACCES.strerror(),
                NULL_ERR_MSG
            )
        );

        // Common without source
        assert_eq!(
            format!("{}", SeccompError::new(Common(TEST_ERR_MSG.into()))),
            TEST_ERR_MSG
        );
        // Common with source
        assert_eq!(
            format!(
                "{}",
                SeccompError::with_source(Common(TEST_ERR_MSG.into()), null_err.clone())
            ),
            format!("{} caused by: {}", TEST_ERR_MSG, NULL_ERR_MSG)
        );

        // Parse without source
        assert_eq!(format!("{}", SeccompError::new(ParseError)), PARSE_ERROR);
        // Parse with source
        assert_eq!(
            format!(
                "{}",
                SeccompError::with_source(ParseError, null_err.clone())
            ),
            format!("{} caused by: {}", PARSE_ERROR, NULL_ERR_MSG)
        );

        // Source
        assert_eq!(
            format!("{}", SeccompError::with_source(ErrorKind::Source, null_err)),
            NULL_ERR_MSG
        );
    }

    #[test]
    fn test_debug() {
        let null_err = CString::new(TEST_NULL_STR).unwrap_err();

        // Errno without source
        assert_eq!(
            format!("{:?}", SeccompError::new(Errno(SeccompErrno::EACCES))),
            format!(
                "Error {{ kind: Errno({}), source: {}, message: \"{}\" }}",
                "EACCES",
                "None",
                SeccompErrno::EACCES.strerror()
            )
        );
        // Errno with source
        assert_eq!(
            format!(
                "{:?}",
                SeccompError::with_source(Errno(SeccompErrno::EACCES), null_err),
            ),
            format!(
                "Error {{ kind: Errno({}), source: {}, message: \"{}\" }}",
                "EACCES",
                "Some(NulError(1, [102, 0, 111, 111]))",
                SeccompErrno::EACCES.strerror()
            )
        );
    }
}
