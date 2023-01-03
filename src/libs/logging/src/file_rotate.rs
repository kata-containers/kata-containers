// Copyright (c) 2020 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Partial code are extracted from
//  https://github.com/sile/sloggers/blob/153c00a59f7218c1d96f522fb7a95c80bb0d530c/src/file.rs
// with following license and copyright.
// The MIT License
//
// Copyright (c) 2017 Takeru Ohta <phjgt308@gmail.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use std::fs::{self, File, OpenOptions};
use std::io::{self, LineWriter, Result, Write};
use std::path::{Path, PathBuf};

/// Default rotate size for logger files.
const DEFAULT_LOG_FILE_SIZE_TO_ROTATE: u64 = 10485760;

/// Default number of log files to keep.
const DEFAULT_HISTORY_LOG_FILES: usize = 3;

/// Writer with file rotation for log files.
///
/// This is a modified version of `FileAppender` from
/// https://github.com/sile/sloggers/blob/153c00a59f7218c1d96f522fb7a95c80bb0d530c/src/file.rs#L190
#[derive(Debug)]
pub struct FileRotator {
    path: PathBuf,
    file: Option<LineWriter<File>>,
    ignore_errors: bool,
    rotate_size: u64,
    rotate_keep: usize,
    truncate: bool,
    written_size: u64,
    #[cfg(test)]
    fail_rename: bool,
}

impl FileRotator {
    /// Create a new instance of [`FileRotator`] to write log file at `path`.
    ///
    /// It returns `std::io::Error` if the path is not a normal file or the parent directory does
    /// not exist.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = Path::new(path.as_ref());
        match p.metadata() {
            Ok(md) => {
                if !md.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("path '{}' is not a file", p.to_string_lossy()),
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        if let Some(parent) = p.parent() {
            if p.has_root() || !parent.as_os_str().is_empty() {
                let md = parent.metadata()?;
                if !md.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("'{}' is not a directory", parent.to_string_lossy()),
                    ));
                }
            }
        }

        Ok(FileRotator {
            path: p.to_path_buf(),
            file: None,
            ignore_errors: false,
            rotate_size: DEFAULT_LOG_FILE_SIZE_TO_ROTATE,
            rotate_keep: DEFAULT_HISTORY_LOG_FILES,
            truncate: false,
            written_size: 0,
            #[cfg(test)]
            fail_rename: false,
        })
    }

    /// Use "truncate" or "append" mode when opening the log file.
    pub fn truncate_mode(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Set the threshold size to rotate log files.
    pub fn rotate_threshold(&mut self, size: u64) -> &mut Self {
        self.rotate_size = size;
        self
    }

    /// Set number of rotated log files to keep.
    pub fn rotate_count(&mut self, count: usize) -> &mut Self {
        self.rotate_keep = count;
        self
    }

    /// Ignore all errors and try best effort to log messages but without guarantee.
    pub fn ignore_errors(&mut self, ignore_errors: bool) -> &mut Self {
        self.ignore_errors = ignore_errors;
        self
    }

    /// Open the log file if
    /// - it hasn't been opened yet.
    /// - current log file has been rotated and needs to open a new log file.
    fn reopen_if_needed(&mut self) -> Result<()> {
        if self.file.is_none() || !self.path.exists() {
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(self.truncate)
                .append(!self.truncate)
                .open(&self.path)?;
            match file.metadata() {
                Ok(md) => self.written_size = md.len(),
                Err(e) => {
                    if self.ignore_errors {
                        // Pretend as an empty file.
                        // It's better to permit over-sized log file instead of disabling rotation.
                        self.written_size = 0;
                    } else {
                        return Err(e);
                    }
                }
            }
            self.file = Some(LineWriter::new(file));
        }

        Ok(())
    }

    /// Try to rotate log files.
    ///
    /// When failed to rotate the log files, we choose to ignore the error instead of possibly
    /// panicking the whole program. This may cause over-sized log files, but that should be easy
    /// to recover.
    fn rotate(&mut self) -> Result<()> {
        for i in (1..=self.rotate_keep).rev() {
            let from = self.rotated_path(i);
            let to = self.rotated_path(i + 1);
            if from.exists() {
                let _ = fs::rename(from, to);
            }
        }

        #[cfg(test)]
        if !self.fail_rename && self.path.exists() {
            let rotated_path = self.rotated_path(1);
            let _ = fs::rename(&self.path, rotated_path);
        }
        #[cfg(not(test))]
        if self.path.exists() {
            let rotated_path = self.rotated_path(1);
            let _ = fs::rename(&self.path, rotated_path);
        }

        let delete_path = self.rotated_path(self.rotate_keep + 1);
        if delete_path.exists() {
            let _ = fs::remove_file(delete_path);
        }

        // Reset the `written_size` so only try to rotate again when another `rotate_size` bytes
        // of log messages have been written to the lo file.
        self.written_size = 0;
        self.reopen_if_needed()?;

        Ok(())
    }

    fn rotated_path(&self, i: usize) -> PathBuf {
        let mut path = self.path.clone().into_os_string();
        path.push(format!(".{}", i));
        PathBuf::from(path)
    }
}

impl Write for FileRotator {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.ignore_errors {
            let _ = self.reopen_if_needed();
            if let Some(file) = self.file.as_mut() {
                let _ = file.write_all(buf);
            }
        } else {
            self.reopen_if_needed()?;
            match self.file.as_mut() {
                Some(file) => file.write_all(buf)?,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Cannot open file: {:?}", self.path),
                    ))
                }
            }
        }

        self.written_size += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(f) = self.file.as_mut() {
            if let Err(e) = f.flush() {
                if !self.ignore_errors {
                    return Err(e);
                }
            }
        }
        if self.written_size >= self.rotate_size {
            if let Err(e) = self.rotate() {
                if !self.ignore_errors {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn test_rotator_valid_path() {
        FileRotator::new("/proc/self").unwrap_err();
        FileRotator::new("/proc/self/__does_not_exist__/log.txt").unwrap_err();

        let _ = FileRotator::new("log.txt").unwrap();
    }

    #[test]
    fn test_rotator_rotate() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut path = tmpdir.path().to_path_buf();
        path.push("log.txt");

        let mut rotator = FileRotator::new(&path).unwrap();
        rotator.truncate_mode(false);
        rotator.rotate_threshold(4);
        rotator.rotate_count(1);
        assert_eq!(rotator.rotate_size, 4);
        assert_eq!(rotator.rotate_keep, 1);
        assert!(!rotator.truncate);

        rotator.write_all("test".as_bytes()).unwrap();
        rotator.flush().unwrap();
        rotator.write_all("test1".as_bytes()).unwrap();
        rotator.flush().unwrap();
        rotator.write_all("t2".as_bytes()).unwrap();
        rotator.flush().unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, "t2");

        let mut path1 = tmpdir.path().to_path_buf();
        path1.push("log.txt.1");
        let content = fs::read_to_string(path1).unwrap();
        assert_eq!(content, "test1");

        let mut path2 = tmpdir.path().to_path_buf();
        path2.push("log.txt.2");
        fs::read_to_string(path2).unwrap_err();
    }

    #[test]
    fn test_rotator_rotate_fail() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut path = tmpdir.path().to_path_buf();
        path.push("log.txt");

        let mut rotator = FileRotator::new(&path).unwrap();
        rotator.truncate_mode(false);
        rotator.rotate_threshold(1);
        rotator.rotate_count(1);
        rotator.fail_rename = true;

        rotator.write_all("test".as_bytes()).unwrap();
        rotator.flush().unwrap();
        let size1 = path.metadata().unwrap().size();

        rotator.write_all("test1".as_bytes()).unwrap();
        rotator.flush().unwrap();
        let size2 = path.metadata().unwrap().size();
        assert!(size2 > size1);

        rotator.write_all("test2".as_bytes()).unwrap();
        rotator.flush().unwrap();
        let size3 = path.metadata().unwrap().size();
        assert!(size3 > size2);
    }
}
