// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Traits for handling file synchronization and length.

use std::fs::File;
use std::io::Result;

/// A trait for flushing the contents of a file to disk.
///
/// This is equivalent to
/// [`std::fd::File::sync_all`](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all)
/// method, but wrapped in a trait so that it can be implemented for other types.
pub trait FileSync {
    /// Flush buffers related to this file to disk.
    fn fsync(&mut self) -> Result<()>;
}

impl FileSync for File {
    fn fsync(&mut self) -> Result<()> {
        self.sync_all()
    }
}

/// A trait for setting the size of a file.
///
/// This is equivalent to
/// [`std::fd::File::set_len`](https://doc.rust-lang.org/std/fs/struct.File.html#method.set_len)
/// method, but wrapped in a trait so that it can be implemented for other types.
pub trait FileSetLen {
    /// Set the size of this file.
    ///
    /// This is the moral equivalent of
    /// [`ftruncate`](http://man7.org/linux/man-pages/man3/ftruncate.3p.html).
    ///
    /// # Arguments
    ///
    /// * `len`: the size to set for file.
    fn set_len(&self, len: u64) -> Result<()>;
}

impl FileSetLen for File {
    fn set_len(&self, len: u64) -> Result<()> {
        File::set_len(self, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};
    use std::path::PathBuf;

    use crate::tempdir::TempDir;

    #[test]
    fn test_fsync() {
        let tempdir = TempDir::new_with_prefix("/tmp/fsync_test").unwrap();
        let mut path = PathBuf::from(tempdir.as_path());
        path.push("file");
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();
        f.write_all(b"Hello, world!").unwrap();
        f.fsync().unwrap();
        assert_eq!(f.metadata().unwrap().len(), 13);
    }

    #[test]
    fn test_set_len() {
        let tempdir = TempDir::new_with_prefix("/tmp/set_len_test").unwrap();
        let mut path = PathBuf::from(tempdir.as_path());
        path.push("file");
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();
        f.set_len(10).unwrap();
        assert_eq!(f.seek(SeekFrom::End(0)).unwrap(), 10);
    }

    #[test]
    fn test_set_len_fails_when_file_not_opened_for_writing() {
        let tempdir = TempDir::new_with_prefix("/tmp/set_len_test").unwrap();
        let mut path = PathBuf::from(tempdir.as_path());
        path.push("file");
        File::create(path.clone()).unwrap();
        let f = OpenOptions::new().read(true).open(&path).unwrap();
        let result = f.set_len(10);
        assert!(result.is_err());
    }
}
