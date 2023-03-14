// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Structures, helpers, and type definitions for working with
//! [`errno`](http://man7.org/linux/man-pages/man3/errno.3.html).

use std::fmt::{Display, Formatter};
use std::io;
use std::result;

/// Wrapper over [`errno`](http://man7.org/linux/man-pages/man3/errno.3.html).
///
/// The error number is an integer number set by system calls and some libc
/// functions in case of error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(i32);

/// A specialized [Result](https://doc.rust-lang.org/std/result/enum.Result.html) type
/// for operations that can return `errno`.
///
/// This typedef is generally used to avoid writing out `errno::Error` directly and is
/// otherwise a direct mapping to `Result`.
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// Creates a new error from the given error number.
    ///
    /// # Arguments
    ///
    /// * `errno`: error number used for creating the `Error`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate libc;
    /// extern crate vmm_sys_util;
    /// #
    /// # use libc;
    /// use vmm_sys_util::errno::Error;
    ///
    /// let err = Error::new(libc::EIO);
    /// ```
    pub fn new(errno: i32) -> Error {
        Error(errno)
    }

    /// Returns the last occurred `errno` wrapped in an `Error`.
    ///
    /// Calling `Error::last()` is the equivalent of using
    /// [`errno`](http://man7.org/linux/man-pages/man3/errno.3.html) in C/C++.
    /// The result of this function only has meaning after a libc call or syscall
    /// where `errno` was set.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate libc;
    /// extern crate vmm_sys_util;
    /// #
    /// # use libc;
    /// # use std::fs::File;
    /// # use std::io::{self, Read};
    /// # use std::env::temp_dir;
    /// use vmm_sys_util::errno::Error;
    /// #
    /// // Reading from a file without permissions returns an error.
    /// let mut path = temp_dir();
    /// path.push("test");
    /// let mut file = File::create(path).unwrap();
    /// let mut buf: Vec<u8> = Vec::new();
    /// assert!(file.read_to_end(&mut buf).is_err());
    ///
    /// // Retrieve the error number of the previous operation using `Error::last()`:
    /// let read_err = Error::last();
    /// #[cfg(unix)]
    /// assert_eq!(read_err, Error::new(libc::EBADF));
    /// #[cfg(not(unix))]
    /// assert_eq!(read_err, Error::new(libc::EIO));
    /// ```
    pub fn last() -> Error {
        // It's safe to unwrap because this `Error` was constructed via `last_os_error`.
        Error(io::Error::last_os_error().raw_os_error().unwrap())
    }

    /// Returns the raw integer value (`errno`) corresponding to this Error.
    ///
    /// # Examples
    /// ```
    /// extern crate vmm_sys_util;
    /// use vmm_sys_util::errno::Error;
    ///
    /// let err = Error::new(13);
    /// assert_eq!(err.errno(), 13);
    /// ```
    pub fn errno(self) -> i32 {
        self.0
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        io::Error::from_raw_os_error(self.0).fmt(f)
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::new(e.raw_os_error().unwrap_or_default())
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        io::Error::from_raw_os_error(err.0)
    }
}

/// Returns the last `errno` as a [`Result`] that is always an error.
///
/// [`Result`]: type.Result.html
pub fn errno_result<T>() -> Result<T> {
    Err(Error::last())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::error::Error as _;
    use std::fs::OpenOptions;
    use std::io::{self, Read};

    #[test]
    pub fn test_errno() {
        #[cfg(unix)]
        let expected_errno = libc::EBADF;
        #[cfg(not(unix))]
        let expected_errno = libc::EIO;

        // try to read from a file without read permissions
        let mut path = temp_dir();
        path.push("test");
        let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .unwrap();
        let mut buf: Vec<u8> = Vec::new();
        assert!(file.read_to_end(&mut buf).is_err());

        // Test that errno_result returns Err and the error is the expected one.
        let last_err = errno_result::<i32>().unwrap_err();
        assert_eq!(last_err, Error::new(expected_errno));

        // Test that the inner value of `Error` corresponds to expected_errno.
        assert_eq!(last_err.errno(), expected_errno);
        assert!(last_err.source().is_none());

        // Test creating an `Error` from a `std::io::Error`.
        assert_eq!(last_err, Error::from(io::Error::last_os_error()));

        // Test that calling `last()` returns the same error as `errno_result()`.
        assert_eq!(last_err, Error::last());

        let last_err: io::Error = last_err.into();
        // Test creating a `std::io::Error` from an `Error`
        assert_eq!(io::Error::last_os_error().kind(), last_err.kind());
    }

    #[test]
    pub fn test_display() {
        // Test the display implementation.
        #[cfg(target_os = "linux")]
        assert_eq!(
            format!("{}", Error::new(libc::EBADF)),
            "Bad file descriptor (os error 9)"
        );
        #[cfg(not(unix))]
        assert_eq!(
            format!("{}", Error::new(libc::EIO)),
            "Access is denied. (os error 5)"
        );
    }
}
