// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon sockets.

use {AsHandleRef, HandleBased, Handle, HandleRef, Peered};
use {sys, Status, ok};

use std::ptr;

/// An object representing a Zircon
/// [socket](https://fuchsia.googlesource.com/zircon/+/master/docs/concepts.md#Message-Passing_Sockets-and-Channels).
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct Socket(Handle);
impl_handle_based!(Socket);
impl Peered for Socket {}

impl Socket {
    /// Create a socket, accessed through a pair of endpoints. Data written
    /// into one may be read from the other.
    ///
    /// Wraps
    /// [zx_socket_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/socket_create.md).
    pub fn create() -> Result<(Socket, Socket), Status> {
        unsafe {
            let mut out0 = 0;
            let mut out1 = 0;
            let opts = 0;
            let status = sys::zx_socket_create(opts, &mut out0, &mut out1);
            ok(status)?;
            Ok((
                Self::from(Handle::from_raw(out0)),
                Self::from(Handle::from_raw(out1))
            ))
        }
    }

    /// Write the given bytes into the socket.
    /// Return value (on success) is number of bytes actually written.
    ///
    /// Wraps
    /// [zx_socket_write](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/socket_write.md).
    pub fn write(&self, bytes: &[u8]) -> Result<usize, Status> {
        let mut actual = 0;
        let opts = 0;
        let status = unsafe {
            sys::zx_socket_write(self.raw_handle(), opts, bytes.as_ptr(), bytes.len(),
                &mut actual)
        };
        ok(status).map(|()| actual)
    }

    /// Read the given bytes from the socket.
    /// Return value (on success) is number of bytes actually read.
    ///
    /// Wraps
    /// [zx_socket_read](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/socket_read.md).
    pub fn read(&self, bytes: &mut [u8]) -> Result<usize, Status> {
        let mut actual = 0;
        let opts = 0;
        let status = unsafe {
            sys::zx_socket_read(self.raw_handle(), opts, bytes.as_mut_ptr(),
                bytes.len(), &mut actual)
        };
        ok(status)
            .map(|()| actual)
            .map_err(|status| {
                // If an error is returned then actual is undefined, so to be safe
                // we set it to 0 and ignore any data that is set in bytes.
                actual = 0;
                status
            })
    }

    /// Close half of the socket, so attempts by the other side to write will fail.
    ///
    /// Implements the `ZX_SOCKET_HALF_CLOSE` option of
    /// [zx_socket_write](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/socket_write.md).
    pub fn half_close(&self) -> Result<(), Status> {
        let status = unsafe { sys::zx_socket_write(self.raw_handle(), sys::ZX_SOCKET_HALF_CLOSE,
            ptr::null(), 0, ptr::null_mut()) };
        ok(status)
    }

    pub fn outstanding_read_bytes(&self) -> Result<usize, Status> {
        let mut outstanding = 0;
        let status = unsafe {
            sys::zx_socket_read(self.raw_handle(), 0, ptr::null_mut(), 0, &mut outstanding)
        };
        ok(status).map(|()| outstanding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_basic() {
        let (s1, s2) = Socket::create().unwrap();

        // Write in one end and read it back out the other.
        assert_eq!(s1.write(b"hello").unwrap(), 5);

        let mut read_vec = vec![0; 8];
        assert_eq!(s2.read(&mut read_vec).unwrap(), 5);
        assert_eq!(&read_vec[0..5], b"hello");

        // Try reading when there is nothing to read.
        assert_eq!(s2.read(&mut read_vec), Err(Status::SHOULD_WAIT));

        // Close the socket from one end.
        assert!(s1.half_close().is_ok());
        assert_eq!(s2.read(&mut read_vec), Err(Status::BAD_STATE));
        assert_eq!(s1.write(b"fail"), Err(Status::BAD_STATE));

        // Writing in the other direction should still work.
        assert_eq!(s1.read(&mut read_vec), Err(Status::SHOULD_WAIT));
        assert_eq!(s2.write(b"back").unwrap(), 4);
        assert_eq!(s1.read(&mut read_vec).unwrap(), 4);
        assert_eq!(&read_vec[0..4], b"back");
    }
}
