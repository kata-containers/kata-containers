// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: (Apache-2.0 AND BSD-3-Clause)

//! Traits for replacing a range with a hole and writing zeroes in a file.

use std::cmp::min;
use std::fs::File;
use std::io::{Error, ErrorKind, Result, Seek, SeekFrom};
use std::os::unix::fs::FileExt;

use crate::fallocate::{fallocate, FallocateMode};

/// A trait for deallocating space in a file.
pub trait PunchHole {
    /// Replace a range of bytes with a hole.
    ///
    /// # Arguments
    ///
    /// * `offset`: offset of the file where to replace with a hole.
    /// * `length`: the number of bytes of the hole to replace with.
    fn punch_hole(&mut self, offset: u64, length: u64) -> Result<()>;
}

impl PunchHole for File {
    fn punch_hole(&mut self, offset: u64, length: u64) -> Result<()> {
        fallocate(self, FallocateMode::PunchHole, true, offset, length as u64)
            .map_err(|e| Error::from_raw_os_error(e.errno()))
    }
}

/// A trait for writing zeroes to a stream.
pub trait WriteZeroes {
    /// Write up to `length` bytes of zeroes to the stream, returning how many bytes were written.
    ///
    /// # Arguments
    ///
    /// * `length`: the number of bytes of zeroes to write to the stream.
    fn write_zeroes(&mut self, length: usize) -> Result<usize>;

    /// Write zeroes to the stream until `length` bytes have been written.
    ///
    /// This method will continuously write zeroes until the requested `length` is satisfied or an
    /// unrecoverable error is encountered.
    ///
    /// # Arguments
    ///
    /// * `length`: the exact number of bytes of zeroes to write to the stream.
    fn write_all_zeroes(&mut self, mut length: usize) -> Result<()> {
        while length > 0 {
            match self.write_zeroes(length) {
                Ok(0) => return Err(Error::from(ErrorKind::WriteZero)),
                Ok(bytes_written) => {
                    length = length
                        .checked_sub(bytes_written)
                        .ok_or_else(|| Error::from(ErrorKind::Other))?
                }
                // If the operation was interrupted, we should retry it.
                Err(e) => {
                    if e.kind() != ErrorKind::Interrupted {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}

/// A trait for writing zeroes to an arbitrary position in a file.
pub trait WriteZeroesAt {
    /// Write up to `length` bytes of zeroes starting at `offset`, returning how many bytes were
    /// written.
    ///
    /// # Arguments
    ///
    /// * `offset`: offset of the file where to write zeroes.
    /// * `length`: the number of bytes of zeroes to write to the stream.
    fn write_zeroes_at(&mut self, offset: u64, length: usize) -> Result<usize>;

    /// Write zeroes starting at `offset` until `length` bytes have been written.
    ///
    /// This method will continuously write zeroes until the requested `length` is satisfied or an
    /// unrecoverable error is encountered.
    ///
    /// # Arguments
    ///
    /// * `offset`: offset of the file where to write zeroes.
    /// * `length`: the exact number of bytes of zeroes to write to the stream.
    fn write_all_zeroes_at(&mut self, mut offset: u64, mut length: usize) -> Result<()> {
        while length > 0 {
            match self.write_zeroes_at(offset, length) {
                Ok(0) => return Err(Error::from(ErrorKind::WriteZero)),
                Ok(bytes_written) => {
                    length = length
                        .checked_sub(bytes_written)
                        .ok_or_else(|| Error::from(ErrorKind::Other))?;
                    offset = offset
                        .checked_add(bytes_written as u64)
                        .ok_or_else(|| Error::from(ErrorKind::Other))?;
                }
                Err(e) => {
                    // If the operation was interrupted, we should retry it.
                    if e.kind() != ErrorKind::Interrupted {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}

impl WriteZeroesAt for File {
    fn write_zeroes_at(&mut self, offset: u64, length: usize) -> Result<usize> {
        // Try to use fallocate() first, since it is more efficient than writing zeroes with
        // write().
        if fallocate(self, FallocateMode::ZeroRange, true, offset, length as u64).is_ok() {
            return Ok(length);
        }

        // Fall back to write().
        // fallocate() failed; fall back to writing a buffer of zeroes until we have written up
        // to `length`.
        let buf_size = min(length, 0x10000);
        let buf = vec![0u8; buf_size];
        let mut num_written: usize = 0;
        while num_written < length {
            let remaining = length - num_written;
            let write_size = min(remaining, buf_size);
            num_written += self.write_at(&buf[0..write_size], offset + num_written as u64)?;
        }
        Ok(length)
    }
}

impl<T: WriteZeroesAt + Seek> WriteZeroes for T {
    fn write_zeroes(&mut self, length: usize) -> Result<usize> {
        let offset = self.seek(SeekFrom::Current(0))?;
        let num_written = self.write_zeroes_at(offset, length)?;
        // Advance the seek cursor as if we had done a real write().
        self.seek(SeekFrom::Current(num_written as i64))?;
        Ok(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{Read, Seek, SeekFrom, Write};

    use crate::tempfile::TempFile;

    #[test]
    fn test_small_write_zeroes() {
        const NON_ZERO_VALUE: u8 = 0x55;
        const BUF_SIZE: usize = 5678;

        let mut f = TempFile::new().unwrap().into_file();
        f.set_len(16384).unwrap();

        // Write buffer of non-zero bytes to offset 1234.
        let orig_data = [NON_ZERO_VALUE; BUF_SIZE];
        f.seek(SeekFrom::Start(1234)).unwrap();
        f.write_all(&orig_data).unwrap();

        // Read back the data plus some overlap on each side.
        let mut readback = [0u8; 16384];
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_exact(&mut readback).unwrap();
        // Bytes before the write should still be 0.
        for read in &readback[0..1234] {
            assert_eq!(*read, 0);
        }
        // Bytes that were just written should have `NON_ZERO_VALUE` value.
        for read in &readback[1234..(1234 + BUF_SIZE)] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
        // Bytes after the written area should still be 0.
        for read in &readback[(1234 + BUF_SIZE)..] {
            assert_eq!(*read, 0);
        }

        // Overwrite some of the data with zeroes.
        f.seek(SeekFrom::Start(2345)).unwrap();
        f.write_all_zeroes(4321).unwrap();
        // Verify seek position after `write_all_zeroes()`.
        assert_eq!(f.seek(SeekFrom::Current(0)).unwrap(), 2345 + 4321);

        // Read back the data and verify that it is now zero.
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_exact(&mut readback).unwrap();
        // Bytes before the write should still be 0.
        for read in &readback[0..1234] {
            assert_eq!(*read, 0);
        }
        // Original data should still exist before the zeroed region.
        for read in &readback[1234..2345] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
        // Verify that `write_all_zeroes()` zeroed the intended region.
        for read in &readback[2345..(2345 + 4321)] {
            assert_eq!(*read, 0);
        }
        // Original data should still exist after the zeroed region.
        for read in &readback[(2345 + 4321)..(1234 + BUF_SIZE)] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
        // The rest of the file should still be 0.
        for read in &readback[(1234 + BUF_SIZE)..] {
            assert_eq!(*read, 0);
        }
    }

    #[test]
    fn test_large_write_zeroes() {
        const NON_ZERO_VALUE: u8 = 0x55;
        const SIZE: usize = 0x2_0000;

        let mut f = TempFile::new().unwrap().into_file();
        f.set_len(16384).unwrap();

        // Write buffer of non-zero bytes. The size of the buffer will be the new
        // size of the file.
        let orig_data = [NON_ZERO_VALUE; SIZE];
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all(&orig_data).unwrap();
        assert_eq!(f.metadata().unwrap().len(), SIZE as u64);

        // Overwrite some of the data with zeroes.
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all_zeroes(0x1_0001).unwrap();
        // Verify seek position after `write_all_zeroes()`.
        assert_eq!(f.seek(SeekFrom::Current(0)).unwrap(), 0x1_0001);

        // Read back the data and verify that it is now zero.
        let mut readback = [0u8; SIZE];
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_exact(&mut readback).unwrap();
        // Verify that `write_all_zeroes()` zeroed the intended region.
        for read in &readback[0..0x1_0001] {
            assert_eq!(*read, 0);
        }
        // Original data should still exist after the zeroed region.
        for read in &readback[0x1_0001..SIZE] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }

        // Now let's zero a certain region by using `write_all_zeroes_at()`.
        f.write_all_zeroes_at(0x1_8001, 0x200).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_exact(&mut readback).unwrap();

        // Original data should still exist before the zeroed region.
        for read in &readback[0x1_0001..0x1_8001] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
        // Verify that `write_all_zeroes_at()` zeroed the intended region.
        for read in &readback[0x1_8001..(0x1_8001 + 0x200)] {
            assert_eq!(*read, 0);
        }
        // Original data should still exist after the zeroed region.
        for read in &readback[(0x1_8001 + 0x200)..SIZE] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
    }

    #[test]
    fn test_punch_hole() {
        const NON_ZERO_VALUE: u8 = 0x55;
        const SIZE: usize = 0x2_0000;

        let mut f = TempFile::new().unwrap().into_file();
        f.set_len(16384).unwrap();

        // Write buffer of non-zero bytes. The size of the buffer will be the new
        // size of the file.
        let orig_data = [NON_ZERO_VALUE; SIZE];
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all(&orig_data).unwrap();
        assert_eq!(f.metadata().unwrap().len(), SIZE as u64);

        // Punch a hole at offset 0x10001.
        // Subsequent reads from this range will return zeros.
        f.punch_hole(0x1_0001, 0x200).unwrap();

        // Read back the data.
        let mut readback = [0u8; SIZE];
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_exact(&mut readback).unwrap();
        // Original data should still exist before the hole.
        for read in &readback[0..0x1_0001] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }
        // Verify that `punch_hole()` zeroed the intended region.
        for read in &readback[0x1_0001..(0x1_0001 + 0x200)] {
            assert_eq!(*read, 0);
        }
        // Original data should still exist after the hole.
        for read in &readback[(0x1_0001 + 0x200)..] {
            assert_eq!(*read, NON_ZERO_VALUE);
        }

        // Punch a hole at the end of the file.
        // Subsequent reads from this range should return zeros.
        f.punch_hole(SIZE as u64 - 0x400, 0x400).unwrap();
        // Even though we punched a hole at the end of the file, the file size should remain the
        // same since FALLOC_FL_PUNCH_HOLE must be used with FALLOC_FL_KEEP_SIZE.
        assert_eq!(f.metadata().unwrap().len(), SIZE as u64);

        let mut readback = [0u8; 0x400];
        f.seek(SeekFrom::Start(SIZE as u64 - 0x400)).unwrap();
        f.read_exact(&mut readback).unwrap();
        // Verify that `punch_hole()` zeroed the intended region.
        for read in &readback[0..0x400] {
            assert_eq!(*read, 0);
        }

        // Punching a hole of len 0 should return an error.
        assert!(f.punch_hole(0x200, 0x0).is_err());
        // Zeroing a region of len 0 should not return an error since we have a fallback path
        // in `write_zeroes_at()` for `fallocate()` failure.
        assert!(f.write_zeroes_at(0x200, 0x0).is_ok());
    }
}
