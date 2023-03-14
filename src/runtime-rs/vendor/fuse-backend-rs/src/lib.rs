// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! A rust library for Fuse(filesystem in userspace) servers and virtio-fs devices.
//!
//! Filesystem in Userspace [`FUSE`](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
//! is a software interface for Unix and Unix-like computer operating systems that lets
//! non-privileged users create their own file systems without editing kernel code.
//! This is achieved by running file system code in user space while the FUSE module provides
//! only a "bridge" to the actual kernel interfaces.
//!
//! On Linux, the FUSE device driver is a general purpose filesystem abstraction layer, which
//! loads as a kernel module and presents a virtual device (/dev/fuse) to communicate with
//! a user (non-kernel) program via a well defined API. The user code need not run with root
//! priviledge if it does not need to access protected data or devices, and can implement
//! a virtual filesystem much more simply than a traditional device driver.
//!
//! In addition to traditional Fuse filesystems, the
//! [virtiofs](https://www.kernel.org/doc/html/latest/filesystems/virtiofs.html)
//! file system for Linux implements a driver for the paravirtualized VIRTIO “virtio-fs” device
//! for guest<->host file system sharing. It allows a guest to mount a directory that has
//! been exported on the host.
//!
//! Virtio-fs uses FUSE as the foundation. Unlike traditional FUSE where the file system daemon
//! runs in userspace, the virtio-fs daemon runs on the host. A VIRTIO device carries FUSE
//! messages and provides extensions for advanced features not available in traditional FUSE.
//! Since the virtio-fs device uses the FUSE protocol for file system requests, the virtiofs
//! file system for Linux is integrated closely with the FUSE file system client. The guest acts
//! as the FUSE client while the host acts as the FUSE server. The /dev/fuse interface between
//! the kernel and userspace is replaced with the virtio-fs device interface.
//!
//! The fuse-backend-rs crate includes several subsystems:
//! * [Fuse API](api/index.html). The Fuse API is the connection between transport layers and file
//!   system drivers. It receives Fuse requests from transport layers, parses the request
//!   according to Fuse ABI, invokes filesystem drivers to server the requests, and eventually
//!   send back the result to the transport layer.
//! * [Fuse ABI](abi/index.html). Currently only Linux Fuse ABIs since v7.27 are supported.
//! * [Transport Layer](transport/index.html). The transport layer receives Fuse requests from
//!   the clients and sends back replies. Currently there are two transport layers are supported:
//!   Linux Fuse device(/dev/fuse) and virtiofs.
//! * Filesystem Drivers. Filesystem drivers implement the concrete Fuse filesystem logic,
//!   at what ever is suitable. A default ["passthrough"](passthrough/index.html) filesystem
//!   driver is implemented as a sample.

extern crate bitflags;
extern crate libc;
#[macro_use]
extern crate log;
extern crate vm_memory;

use std::ffi::{CStr, FromBytesWithNulError};
use std::io::ErrorKind;
use std::{error, fmt, io};

use vm_memory::bitmap::BitmapSlice;

/// Error codes for Fuse related operations.
#[derive(Debug)]
pub enum Error {
    /// Failed to decode protocol messages.
    DecodeMessage(io::Error),
    /// Failed to encode protocol messages.
    EncodeMessage(io::Error),
    /// One or more parameters are missing.
    MissingParameter,
    /// A C string parameter is invalid.
    InvalidCString(FromBytesWithNulError),
    /// The `len` field of the header is too small.
    InvalidHeaderLength,
    /// The `size` field of the `SetxattrIn` message does not match the length
    /// of the decoded value.
    InvalidXattrSize((u32, usize)),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            DecodeMessage(err) => write!(f, "failed to decode fuse message: {}", err),
            EncodeMessage(err) => write!(f, "failed to encode fuse message: {}", err),
            MissingParameter => write!(f, "one or more parameters are missing"),
            InvalidHeaderLength => write!(f, "the `len` field of the header is too small"),
            InvalidCString(err) => write!(f, "a c string parameter is invalid: {}", err),
            InvalidXattrSize((size, len)) => write!(
                f,
                "The `size` field of the `SetxattrIn` message does not match the length of the \
                 decoded value: size = {}, value.len() = {}",
                size, len
            ),
        }
    }
}

/// Result for Fuse related operations.
pub type Result<T> = ::std::result::Result<T, Error>;

pub mod abi;
pub mod api;

#[cfg(all(any(feature = "fusedev", feature = "virtiofs"), target_os = "linux"))]
pub mod passthrough;
pub mod transport;

pub mod common;
pub use self::common::*;

/// Convert io::ErrorKind to OS error code.
/// Reference to libstd/sys/unix/mod.rs => decode_error_kind.
pub fn encode_io_error_kind(kind: ErrorKind) -> i32 {
    match kind {
        //ErrorKind::ConnectionRefused => libc::ECONNREFUSED,
        //ErrorKind::ConnectionReset => libc::ECONNRESET,
        ErrorKind::PermissionDenied => libc::EPERM | libc::EACCES,
        //ErrorKind::BrokenPipe => libc::EPIPE,
        //ErrorKind::NotConnected => libc::ENOTCONN,
        //ErrorKind::ConnectionAborted => libc::ECONNABORTED,
        //ErrorKind::AddrNotAvailable => libc::EADDRNOTAVAIL,
        //ErrorKind::AddrInUse => libc::EADDRINUSE,
        ErrorKind::NotFound => libc::ENOENT,
        ErrorKind::Interrupted => libc::EINTR,
        //ErrorKind::InvalidInput => libc::EINVAL,
        //ErrorKind::TimedOut => libc::ETIMEDOUT,
        ErrorKind::AlreadyExists => libc::EEXIST,
        ErrorKind::WouldBlock => libc::EWOULDBLOCK,
        _ => libc::EIO,
    }
}

/// trim all trailing nul terminators.
pub fn bytes_to_cstr(buf: &[u8]) -> Result<&CStr> {
    // There might be multiple 0s at the end of buf, find & use the first one and trim other zeros.
    match buf.iter().position(|x| *x == 0) {
        // Convert to a `CStr` so that we can drop the '\0' byte at the end and make sure
        // there are no interior '\0' bytes.
        Some(pos) => CStr::from_bytes_with_nul(&buf[0..=pos]).map_err(Error::InvalidCString),
        None => {
            // Invalid input, just call CStr::from_bytes_with_nul() for suitable error code
            CStr::from_bytes_with_nul(buf).map_err(Error::InvalidCString)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_cstr() {
        assert_eq!(
            bytes_to_cstr(&[0x1u8, 0x2u8, 0x0]).unwrap(),
            CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x1u8, 0x2u8, 0x0, 0x0]).unwrap(),
            CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x1u8, 0x2u8, 0x0, 0x1]).unwrap(),
            CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x1u8, 0x2u8, 0x0, 0x0, 0x1]).unwrap(),
            CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x1u8, 0x2u8, 0x0, 0x1, 0x0]).unwrap(),
            CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap()
        );

        assert_eq!(
            bytes_to_cstr(&[0x0u8, 0x2u8, 0x0]).unwrap(),
            CStr::from_bytes_with_nul(&[0x0u8]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x0u8, 0x0]).unwrap(),
            CStr::from_bytes_with_nul(&[0x0u8]).unwrap()
        );
        assert_eq!(
            bytes_to_cstr(&[0x0u8]).unwrap(),
            CStr::from_bytes_with_nul(&[0x0u8]).unwrap()
        );

        bytes_to_cstr(&[0x1u8]).unwrap_err();
        bytes_to_cstr(&[0x1u8, 0x1]).unwrap_err();
    }

    #[test]
    fn test_encode_io_error_kind() {
        assert_eq!(encode_io_error_kind(ErrorKind::NotFound), libc::ENOENT);
        assert_eq!(encode_io_error_kind(ErrorKind::Interrupted), libc::EINTR);
        assert_eq!(encode_io_error_kind(ErrorKind::AlreadyExists), libc::EEXIST);
        assert_eq!(
            encode_io_error_kind(ErrorKind::WouldBlock),
            libc::EWOULDBLOCK
        );
        assert_eq!(encode_io_error_kind(ErrorKind::TimedOut), libc::EIO);
    }
}
