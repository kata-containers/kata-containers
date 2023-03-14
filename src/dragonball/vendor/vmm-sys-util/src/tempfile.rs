// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Struct for handling temporary files as well as any cleanup required.
//!
//! The temporary files will be created with a name available as well as having
//! an exposed `fs::File` for reading/writing.
//!
//! The file will be removed when the object goes out of scope.
//!
//! # Examples
//!
//! ```
//! use std::env::temp_dir;
//! use std::io::Write;
//! use std::path::{Path, PathBuf};
//! use vmm_sys_util::tempfile::TempFile;
//!
//! let mut prefix = temp_dir();
//! prefix.push("tempfile");
//! let t = TempFile::new_with_prefix(prefix).unwrap();
//! let mut f = t.as_file();
//! f.write_all(b"hello world").unwrap();
//! f.sync_all().unwrap();

use std::env::temp_dir;
use std::ffi::OsStr;
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::errno::{Error, Result};
use crate::rand::rand_alphanumerics;

/// Wrapper for working with temporary files.
///
/// The file will be maintained for the lifetime of the `TempFile` object.
pub struct TempFile {
    path: PathBuf,
    file: Option<File>,
}

impl TempFile {
    /// Creates the TempFile using a prefix.
    ///
    /// # Arguments
    ///
    /// `prefix`: The path and filename where to create the temporary file. Six
    /// random alphanumeric characters will be added to the end of this to form
    /// the filename.
    pub fn new_with_prefix<P: AsRef<OsStr>>(prefix: P) -> Result<TempFile> {
        let file_path_str = format!(
            "{}{}",
            prefix.as_ref().to_str().unwrap_or_default(),
            rand_alphanumerics(6).to_str().unwrap_or_default()
        );
        let file_path_buf = PathBuf::from(&file_path_str);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path_buf.as_path())?;

        Ok(TempFile {
            path: file_path_buf,
            file: Some(file),
        })
    }

    /// Creates the TempFile inside a specific location.
    ///
    /// # Arguments
    ///
    /// `path`: The path where to create a temporary file with a filename formed from
    /// six random alphanumeric characters.
    pub fn new_in(path: &Path) -> Result<Self> {
        let mut path_buf = path.canonicalize().unwrap();
        // This `push` adds a trailing slash ("/whatever/path" -> "/whatever/path/").
        // This is safe for paths with an already existing trailing slash.
        path_buf.push("");
        let temp_file = TempFile::new_with_prefix(path_buf.as_path())?;
        Ok(temp_file)
    }

    /// Creates the TempFile.
    ///
    /// Creates a temporary file inside `$TMPDIR` if set, otherwise inside `/tmp`.
    /// The filename will consist of six random alphanumeric characters.
    pub fn new() -> Result<Self> {
        let in_tmp_dir = temp_dir();
        let temp_file = TempFile::new_in(in_tmp_dir.as_path())?;
        Ok(temp_file)
    }

    /// Removes the temporary file.
    ///
    /// Calling this is optional as dropping a `TempFile` object will also
    /// remove the file.  Calling remove explicitly allows for better error
    /// handling.
    pub fn remove(&mut self) -> Result<()> {
        fs::remove_file(&self.path).map_err(Error::from)
    }

    /// Returns the path to the file if the `TempFile` object that is wrapping the file
    /// is still in scope.
    ///
    /// If we remove the file by explicitly calling [`remove`](#method.remove),
    /// `as_path()` can still be used to return the path to that file (even though that
    /// path does not point at an existing entity anymore).
    /// Calling `as_path()` after `remove()` is useful, for example, when you need a
    /// random path string, but don't want an actual resource at that path.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Returns a reference to the File.
    pub fn as_file(&self) -> &File {
        // It's safe to unwrap because `file` can be `None` only after calling `into_file`
        // which consumes this object.
        self.file.as_ref().unwrap()
    }

    /// Consumes the TempFile, returning the wrapped file.
    ///
    /// This also removes the file from the system. The file descriptor remains opened and
    /// it can be used until the returned file is dropped.
    pub fn into_file(mut self) -> File {
        self.file.take().unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn test_create_file_with_prefix() {
        fn between(lower: u8, upper: u8, to_check: u8) -> bool {
            (to_check >= lower) && (to_check <= upper)
        }

        let mut prefix = temp_dir();
        prefix.push("asdf");
        let t = TempFile::new_with_prefix(&prefix).unwrap();
        let path = t.as_path().to_owned();

        // Check filename exists
        assert!(path.is_file());

        // Check filename is in the correct location
        assert!(path.starts_with(temp_dir()));

        // Check filename has random added
        assert_eq!(path.file_name().unwrap().to_string_lossy().len(), 10);

        // Check filename has only ascii letters / numbers
        for n in path.file_name().unwrap().to_string_lossy().bytes() {
            assert!(between(b'0', b'9', n) || between(b'a', b'z', n) || between(b'A', b'Z', n));
        }

        // Check we can write to the file
        let mut f = t.as_file();
        f.write_all(b"hello world").unwrap();
        f.sync_all().unwrap();
        assert_eq!(f.metadata().unwrap().len(), 11);
    }

    #[test]
    fn test_create_file_new() {
        let t = TempFile::new().unwrap();
        let path = t.as_path().to_owned();

        // Check filename is in the correct location
        assert!(path.starts_with(temp_dir().canonicalize().unwrap()));
    }

    #[test]
    fn test_create_file_new_in() {
        let t = TempFile::new_in(temp_dir().as_path()).unwrap();
        let path = t.as_path().to_owned();

        // Check filename exists
        assert!(path.is_file());

        // Check filename is in the correct location
        assert!(path.starts_with(temp_dir().canonicalize().unwrap()));

        let t = TempFile::new_in(temp_dir().as_path()).unwrap();
        let path = t.as_path().to_owned();

        // Check filename is in the correct location
        assert!(path.starts_with(temp_dir().canonicalize().unwrap()));
    }

    #[test]
    fn test_remove_file() {
        let mut prefix = temp_dir();
        prefix.push("asdf");

        let mut t = TempFile::new_with_prefix(prefix).unwrap();
        let path = t.as_path().to_owned();

        // Check removal.
        assert!(t.remove().is_ok());
        assert!(!path.exists());

        // Calling `as_path()` after the file was removed is allowed.
        let path_2 = t.as_path().to_owned();
        assert_eq!(path, path_2);

        // Check trying to remove a second time returns an error.
        assert!(t.remove().is_err());
    }

    #[test]
    fn test_drop_file() {
        let mut prefix = temp_dir();
        prefix.push("asdf");

        let t = TempFile::new_with_prefix(prefix).unwrap();
        let path = t.as_path().to_owned();

        assert!(path.starts_with(temp_dir()));
        drop(t);
        assert!(!path.exists());
    }

    #[test]
    fn test_into_file() {
        let mut prefix = temp_dir();
        prefix.push("asdf");

        let text = b"hello world";
        let temp_file = TempFile::new_with_prefix(prefix).unwrap();
        let path = temp_file.as_path().to_owned();
        fs::write(path, text).unwrap();

        let mut file = temp_file.into_file();
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, text);
    }
}
