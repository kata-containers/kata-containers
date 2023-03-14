//! Rust friendly bindings to the various *nix system functions.
//!
//! Modules are structured according to the C header file that they would be
//! defined in.
#![crate_name = "nix"]
#![cfg(unix)]
#![allow(non_camel_case_types)]
// latest bitflags triggers a rustc bug with cross-crate macro expansions causing dead_code
// warnings even though the macro expands into something with allow(dead_code)
#![allow(dead_code)]
#![cfg_attr(test, deny(warnings))]
#![recursion_limit = "500"]
#![deny(unused)]
#![deny(unstable_features)]
#![deny(missing_copy_implementations)]
#![deny(missing_debug_implementations)]

// Re-exported external crates
pub use libc;

// Private internal modules
#[macro_use] mod macros;

// Public crates
#[cfg(not(target_os = "redox"))]
pub mod dir;
pub mod env;
pub mod errno;
#[deny(missing_docs)]
pub mod features;
pub mod fcntl;
#[deny(missing_docs)]
#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "illumos",
          target_os = "openbsd"))]
pub mod ifaddrs;
#[cfg(any(target_os = "android",
          target_os = "linux"))]
pub mod kmod;
#[cfg(any(target_os = "android",
          target_os = "freebsd",
          target_os = "linux"))]
pub mod mount;
#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "fushsia",
          target_os = "linux",
          target_os = "netbsd"))]
pub mod mqueue;
#[deny(missing_docs)]
#[cfg(not(target_os = "redox"))]
pub mod net;
#[deny(missing_docs)]
pub mod poll;
#[deny(missing_docs)]
#[cfg(not(any(target_os = "redox", target_os = "fuchsia")))]
pub mod pty;
pub mod sched;
pub mod sys;
pub mod time;
// This can be implemented for other platforms as soon as libc
// provides bindings for them.
#[cfg(all(target_os = "linux",
          any(target_arch = "x86", target_arch = "x86_64")))]
pub mod ucontext;
pub mod unistd;

/*
 *
 * ===== Result / Error =====
 *
 */

use libc::{c_char, PATH_MAX};

use std::{ptr, result};
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use errno::Errno;

/// Nix Result Type
pub type Result<T> = result::Result<T, Errno>;

/// Nix's main error type.
///
/// It's a wrapper around Errno.  As such, it's very interoperable with
/// [`std::io::Error`], but it has the advantages of:
/// * `Clone`
/// * `Copy`
/// * `Eq`
/// * Small size
/// * Represents all of the system's errnos, instead of just the most common
/// ones.
pub type Error = Errno;

pub trait NixPath {
    fn is_empty(&self) -> bool;

    fn len(&self) -> usize;

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
        where F: FnOnce(&CStr) -> T;
}

impl NixPath for str {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(OsStr::new(self))
    }

    fn len(&self) -> usize {
        NixPath::len(OsStr::new(self))
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
        where F: FnOnce(&CStr) -> T {
            OsStr::new(self).with_nix_path(f)
        }
}

impl NixPath for OsStr {
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
        where F: FnOnce(&CStr) -> T {
            self.as_bytes().with_nix_path(f)
        }
}

impl NixPath for CStr {
    fn is_empty(&self) -> bool {
        self.to_bytes().is_empty()
    }

    fn len(&self) -> usize {
        self.to_bytes().len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
            where F: FnOnce(&CStr) -> T {
        // Equivalence with the [u8] impl.
        if self.len() >= PATH_MAX as usize {
            return Err(Error::from(Errno::ENAMETOOLONG))
        }

        Ok(f(self))
    }
}

impl NixPath for [u8] {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T>
            where F: FnOnce(&CStr) -> T {
        let mut buf = [0u8; PATH_MAX as usize];

        if self.len() >= PATH_MAX as usize {
            return Err(Error::from(Errno::ENAMETOOLONG))
        }

        match self.iter().position(|b| *b == 0) {
            Some(_) => Err(Error::from(Errno::EINVAL)),
            None => {
                unsafe {
                    // TODO: Replace with bytes::copy_memory. rust-lang/rust#24028
                    ptr::copy_nonoverlapping(self.as_ptr(), buf.as_mut_ptr(), self.len());
                    Ok(f(CStr::from_ptr(buf.as_ptr() as *const c_char)))
                }

            }
        }
    }
}

impl NixPath for Path {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(self.as_os_str())
    }

    fn len(&self) -> usize {
        NixPath::len(self.as_os_str())
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T> where F: FnOnce(&CStr) -> T {
        self.as_os_str().with_nix_path(f)
    }
}

impl NixPath for PathBuf {
    fn is_empty(&self) -> bool {
        NixPath::is_empty(self.as_os_str())
    }

    fn len(&self) -> usize {
        NixPath::len(self.as_os_str())
    }

    fn with_nix_path<T, F>(&self, f: F) -> Result<T> where F: FnOnce(&CStr) -> T {
        self.as_os_str().with_nix_path(f)
    }
}
