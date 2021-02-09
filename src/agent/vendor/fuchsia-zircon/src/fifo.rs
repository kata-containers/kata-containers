// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon fifo objects.

use {AsHandleRef, HandleBased, Handle, HandleRef, Status};
use {sys, ok};

/// An object representing a Zircon fifo.
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct Fifo(Handle);
impl_handle_based!(Fifo);

impl Fifo {
    /// Create a pair of fifos and return their endpoints. Writing to one endpoint enqueues an
    /// element into the fifo from which the opposing endpoint reads. Wraps the
    /// [zx_fifo_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/fifo_create.md)
    /// syscall.
    pub fn create(elem_count: u32, elem_size: u32)
        -> Result<(Fifo, Fifo), Status>
    {
        let mut out0 = 0;
        let mut out1 = 0;
        let options = 0;
        let status = unsafe {
            sys::zx_fifo_create(elem_count, elem_size, options, &mut out0, &mut out1)
        };
        ok(status)?;
        unsafe { Ok((
            Self::from(Handle::from_raw(out0)),
            Self::from(Handle::from_raw(out1))
        ))}
    }

    /// Attempts to write some number of elements into the fifo. The number of bytes written will be
    /// rounded down to a multiple of the fifo's element size.
    /// Return value (on success) is number of elements actually written.
    ///
    /// Wraps
    /// [zx_fifo_write](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/fifo_write.md).
    pub fn write(&self, bytes: &[u8]) -> Result<u32, Status> {
        let mut num_entries_written = 0;
        let status = unsafe {
            sys::zx_fifo_write(self.raw_handle(), bytes.as_ptr(), bytes.len(),
                &mut num_entries_written)
        };
        ok(status).map(|()| num_entries_written)
    }

    /// Attempts to read some number of elements out of the fifo. The number of bytes read will
    /// always be a multiple of the fifo's element size.
    /// Return value (on success) is number of elements actually read.
    ///
    /// Wraps
    /// [zx_fifo_read](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/fifo_read.md).
    pub fn read(&self, bytes: &mut [u8]) -> Result<u32, Status> {
        let mut num_entries_read = 0;
        let status = unsafe {
            sys::zx_fifo_read(self.raw_handle(), bytes.as_mut_ptr(), bytes.len(),
                &mut num_entries_read)
        };
        ok(status).map(|()| num_entries_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_basic() {
        let (fifo1, fifo2) = Fifo::create(4, 2).unwrap();

        // Trying to write less than one element should fail.
        assert_eq!(fifo1.write(b""), Err(Status::OUT_OF_RANGE));
        assert_eq!(fifo1.write(b"h"), Err(Status::OUT_OF_RANGE));

        // Should write one element "he" and ignore the last half-element as it rounds down.
        assert_eq!(fifo1.write(b"hex").unwrap(), 1);

        // Should write three elements "ll" "o " "wo" and drop the rest as it is full.
        assert_eq!(fifo1.write(b"llo worlds").unwrap(), 3);

        // Now that the fifo is full any further attempts to write should fail.
        assert_eq!(fifo1.write(b"blah blah"), Err(Status::SHOULD_WAIT));

        // Read all 4 entries from the other end.
        let mut read_vec = vec![0; 8];
        assert_eq!(fifo2.read(&mut read_vec).unwrap(), 4);
        assert_eq!(read_vec, b"hello wo");

        // Reading again should fail as the fifo is empty.
        assert_eq!(fifo2.read(&mut read_vec), Err(Status::SHOULD_WAIT));
    }
}
