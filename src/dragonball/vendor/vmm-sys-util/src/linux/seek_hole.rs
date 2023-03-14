// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Traits and implementations over [lseek64](https://linux.die.net/man/3/lseek64).

use std::fs::File;
use std::io::{Error, Result};
use std::os::unix::io::AsRawFd;

#[cfg(target_env = "musl")]
use libc::{c_int, lseek64, ENXIO};

#[cfg(target_env = "gnu")]
use libc::{lseek64, ENXIO, SEEK_DATA, SEEK_HOLE};

#[cfg(all(not(target_env = "musl"), target_os = "android"))]
use libc::{lseek64, ENXIO, SEEK_DATA, SEEK_HOLE};

/// A trait for seeking to the next hole or non-hole position in a file.
pub trait SeekHole {
    /// Seek to the first hole in a file.
    ///
    /// Seek at a position greater than or equal to `offset`. If no holes exist
    /// after `offset`, the seek position will be set to the end of the file.
    /// If `offset` is at or after the end of the file, the seek position is
    /// unchanged, and None is returned.
    ///
    /// Returns the current seek position after the seek or an error.
    fn seek_hole(&mut self, offset: u64) -> Result<Option<u64>>;

    /// Seek to the first data in a file.
    ///
    /// Seek at a position greater than or equal to `offset`.
    /// If no data exists after `offset`, the seek position is unchanged,
    /// and None is returned.
    ///
    /// Returns the current offset after the seek or an error.
    fn seek_data(&mut self, offset: u64) -> Result<Option<u64>>;
}

#[cfg(target_env = "musl")]
const SEEK_DATA: c_int = 3;
#[cfg(target_env = "musl")]
const SEEK_HOLE: c_int = 4;

// Safe wrapper for `libc::lseek64()`
fn lseek(file: &mut File, offset: i64, whence: i32) -> Result<Option<u64>> {
    // This is safe because we pass a known-good file descriptor.
    let res = unsafe { lseek64(file.as_raw_fd(), offset, whence) };

    if res < 0 {
        // Convert ENXIO into None; pass any other error as-is.
        let err = Error::last_os_error();
        if let Some(errno) = Error::raw_os_error(&err) {
            if errno == ENXIO {
                return Ok(None);
            }
        }
        Err(err)
    } else {
        Ok(Some(res as u64))
    }
}

impl SeekHole for File {
    fn seek_hole(&mut self, offset: u64) -> Result<Option<u64>> {
        lseek(self, offset as i64, SEEK_HOLE)
    }

    fn seek_data(&mut self, offset: u64) -> Result<Option<u64>> {
        lseek(self, offset as i64, SEEK_DATA)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::TempDir;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};
    use std::path::PathBuf;

    fn seek_cur(file: &mut File) -> u64 {
        file.seek(SeekFrom::Current(0)).unwrap()
    }

    #[test]
    fn seek_data() {
        let tempdir = TempDir::new_with_prefix("/tmp/seek_data_test").unwrap();
        let mut path = PathBuf::from(tempdir.as_path());
        path.push("test_file");
        let mut file = File::create(&path).unwrap();

        // Empty file
        assert_eq!(file.seek_data(0).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // File with non-zero length consisting entirely of a hole
        file.set_len(0x10000).unwrap();
        assert_eq!(file.seek_data(0).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // seek_data at or after the end of the file should return None
        assert_eq!(file.seek_data(0x10000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);
        assert_eq!(file.seek_data(0x10001).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // Write some data to [0x10000, 0x20000)
        let b = [0x55u8; 0x10000];
        file.seek(SeekFrom::Start(0x10000)).unwrap();
        file.write_all(&b).unwrap();
        assert_eq!(file.seek_data(0).unwrap(), Some(0x10000));
        assert_eq!(seek_cur(&mut file), 0x10000);

        // seek_data within data should return the same offset
        assert_eq!(file.seek_data(0x10000).unwrap(), Some(0x10000));
        assert_eq!(seek_cur(&mut file), 0x10000);
        assert_eq!(file.seek_data(0x10001).unwrap(), Some(0x10001));
        assert_eq!(seek_cur(&mut file), 0x10001);
        assert_eq!(file.seek_data(0x1FFFF).unwrap(), Some(0x1FFFF));
        assert_eq!(seek_cur(&mut file), 0x1FFFF);

        // Extend the file to add another hole after the data
        file.set_len(0x30000).unwrap();
        assert_eq!(file.seek_data(0).unwrap(), Some(0x10000));
        assert_eq!(seek_cur(&mut file), 0x10000);
        assert_eq!(file.seek_data(0x1FFFF).unwrap(), Some(0x1FFFF));
        assert_eq!(seek_cur(&mut file), 0x1FFFF);
        assert_eq!(file.seek_data(0x20000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0x1FFFF);
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn seek_hole() {
        let tempdir = TempDir::new_with_prefix("/tmp/seek_hole_test").unwrap();
        let mut path = PathBuf::from(tempdir.as_path());
        path.push("test_file");
        let mut file = File::create(&path).unwrap();

        // Empty file
        assert_eq!(file.seek_hole(0).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // File with non-zero length consisting entirely of a hole
        file.set_len(0x10000).unwrap();
        assert_eq!(file.seek_hole(0).unwrap(), Some(0));
        assert_eq!(seek_cur(&mut file), 0);
        assert_eq!(file.seek_hole(0xFFFF).unwrap(), Some(0xFFFF));
        assert_eq!(seek_cur(&mut file), 0xFFFF);

        // seek_hole at or after the end of the file should return None
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x10000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);
        assert_eq!(file.seek_hole(0x10001).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // Write some data to [0x10000, 0x20000)
        let b = [0x55u8; 0x10000];
        file.seek(SeekFrom::Start(0x10000)).unwrap();
        file.write_all(&b).unwrap();

        // seek_hole within a hole should return the same offset
        assert_eq!(file.seek_hole(0).unwrap(), Some(0));
        assert_eq!(seek_cur(&mut file), 0);
        assert_eq!(file.seek_hole(0xFFFF).unwrap(), Some(0xFFFF));
        assert_eq!(seek_cur(&mut file), 0xFFFF);

        // seek_hole within data should return the next hole (EOF)
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x10000).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x10001).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x1FFFF).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);

        // seek_hole at EOF after data should return None
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x20000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // Extend the file to add another hole after the data
        file.set_len(0x30000).unwrap();
        assert_eq!(file.seek_hole(0).unwrap(), Some(0));
        assert_eq!(seek_cur(&mut file), 0);
        assert_eq!(file.seek_hole(0xFFFF).unwrap(), Some(0xFFFF));
        assert_eq!(seek_cur(&mut file), 0xFFFF);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x10000).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x1FFFF).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x20000).unwrap(), Some(0x20000));
        assert_eq!(seek_cur(&mut file), 0x20000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x20001).unwrap(), Some(0x20001));
        assert_eq!(seek_cur(&mut file), 0x20001);

        // seek_hole at EOF after a hole should return None
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x30000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);

        // Write some data to [0x20000, 0x30000)
        file.seek(SeekFrom::Start(0x20000)).unwrap();
        file.write_all(&b).unwrap();

        // seek_hole within [0x20000, 0x30000) should now find the hole at EOF
        assert_eq!(file.seek_hole(0x20000).unwrap(), Some(0x30000));
        assert_eq!(seek_cur(&mut file), 0x30000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x20001).unwrap(), Some(0x30000));
        assert_eq!(seek_cur(&mut file), 0x30000);
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file.seek_hole(0x30000).unwrap(), None);
        assert_eq!(seek_cur(&mut file), 0);
    }
}
