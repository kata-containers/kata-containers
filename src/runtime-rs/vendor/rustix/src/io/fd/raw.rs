//! The following is derived from Rust's
//! library/std/src/os/fd/raw.rs at revision
//! dca3f1b786efd27be3b325ed1e01e247aa589c3b.
//!
//! Raw Unix-like file descriptors.

#![cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
#![allow(unsafe_code)]

use crate::imp::c;

/// Raw file descriptors.
#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
pub type RawFd = c::c_int;

/// A trait to extract the raw file descriptor from an underlying object.
///
/// This is only available on unix and WASI platforms and must be imported in
/// order to call the method. Windows platforms have a corresponding
/// `AsRawHandle` and `AsRawSocket` set of traits.
#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
pub trait AsRawFd {
    /// Extracts the raw file descriptor.
    ///
    /// This method does **not** pass ownership of the raw file descriptor
    /// to the caller. The descriptor is only guaranteed to be valid while
    /// the original object has not yet been destroyed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(unix)]
    /// use std::os::unix::io::{AsRawFd, RawFd};
    /// #[cfg(target_os = "wasi")]
    /// use std::os::wasi::io::{AsRawFd, RawFd};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// // Note that `raw_fd` is only valid as long as `f` exists.
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.as_raw_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
    fn as_raw_fd(&self) -> RawFd;
}

/// A trait to express the ability to construct an object from a raw file
/// descriptor.
#[cfg_attr(staged_api, stable(feature = "from_raw_os", since = "1.1.0"))]
pub trait FromRawFd {
    /// Constructs a new instance of `Self` from the given raw file
    /// descriptor.
    ///
    /// This function **consumes ownership** of the specified file
    /// descriptor. The returned object will take responsibility for closing
    /// it when the object goes out of scope.
    ///
    /// This function is also unsafe as the primitives currently returned
    /// have the contract that they are the sole owner of the file
    /// descriptor they are wrapping. Usage of this function could
    /// accidentally allow violating this contract which can cause memory
    /// unsafety in code that relies on it being true.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(unix)]
    /// use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
    /// #[cfg(target_os = "wasi")]
    /// use std::os::wasi::io::{FromRawFd, IntoRawFd, RawFd};
    ///
    /// let f = File::open("foo.txt")?;
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.into_raw_fd();
    /// // SAFETY: no other functions should call `from_raw_fd`, so there
    /// // is only one owner for the file descriptor.
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let f = unsafe { File::from_raw_fd(raw_fd) };
    /// # Ok::<(), io::Error>(())
    /// ```
    #[cfg_attr(staged_api, stable(feature = "from_raw_os", since = "1.1.0"))]
    unsafe fn from_raw_fd(fd: RawFd) -> Self;
}

/// A trait to express the ability to consume an object and acquire ownership of
/// its raw file descriptor.
#[cfg_attr(staged_api, stable(feature = "into_raw_os", since = "1.4.0"))]
pub trait IntoRawFd {
    /// Consumes this object, returning the raw underlying file descriptor.
    ///
    /// This function **transfers ownership** of the underlying file descriptor
    /// to the caller. Callers are then the unique owners of the file descriptor
    /// and must close the descriptor once it's no longer needed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(unix)]
    /// use std::os::unix::io::{IntoRawFd, RawFd};
    /// #[cfg(target_os = "wasi")]
    /// use std::os::wasi::io::{IntoRawFd, RawFd};
    ///
    /// let f = File::open("foo.txt")?;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.into_raw_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[cfg_attr(staged_api, stable(feature = "into_raw_os", since = "1.4.0"))]
    fn into_raw_fd(self) -> RawFd;
}

#[cfg_attr(
    staged_api,
    stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")
)]
impl AsRawFd for RawFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        *self
    }
}
#[cfg_attr(
    staged_api,
    stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")
)]
impl IntoRawFd for RawFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self
    }
}
#[cfg_attr(
    staged_api,
    stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")
)]
impl FromRawFd for RawFd {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> RawFd {
        fd
    }
}
