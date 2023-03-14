// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! RAFS: a chunk dedup, on-demand loading, readonly fuse filesystem.
//!
//! The Rafs filesystem is blob based readonly filesystem with chunk deduplication. A Rafs
//! filesystem is composed up of a metadata blob and zero or more data blobs. A blob is just a
//! plain object containing data chunks. Data chunks may be compressed, encrypted and deduplicated
//! by chunk content digest value. When Rafs file is used for container images, Rafs metadata blob
//! contains all filesystem metadatas, such as directory, file name, permission etc. Actually file
//! contents are divided into chunks and stored into data blobs. Rafs may built one data blob for
//! each container image layer or build a single data blob for the whole image, according to
//! building options.
//!
//! There are several versions of Rafs filesystem defined:
//! - V4: the original Rafs filesystem format
//! - V5: an optimized version based on V4 with metadata direct mapping, data prefetching etc.
//! - V6: a redesigned version to reduce metadata blob size and inter-operable with in kernel erofs,
//!   better support of virtio-fs.
//!
//! The nydus-rafs crate depends on the nydus-storage crate to access metadata and data blobs and
//! improve performance by caching data on local storage. The nydus-rafs itself includes two main
//! sub modules:
//! - [fs](fs/index.html): the Rafs core to glue fuse, storage backend and filesystem metadata.
//! - [metadata](metadata/index.html): defines and accesses Rafs filesystem metadata.
//!
//! For more information, please refer to
//! [Dragonfly Image Service](https://github.com/dragonflyoss/image-service)

#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate nydus_error;
#[macro_use]
extern crate nydus_storage as storage;

use std::any::Any;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::fs::File;
use std::io::{BufWriter, Error, Read, Result, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::metadata::{RafsInode, RafsSuper};

pub mod fs;
pub mod metadata;
#[cfg(test)]
pub mod mock;

/// Error codes for rafs related operations.
#[derive(Debug)]
pub enum RafsError {
    Unsupported,
    Uninitialized,
    AlreadyMounted,
    ReadMetadata(Error, String),
    LoadConfig(Error),
    ParseConfig(serde_json::Error),
    SwapBackend(Error),
    FillSuperblock(Error),
    CreateDevice(Error),
    Prefetch(String),
    Configure(String),
    Incompatible(u16),
    IllegalMetaStruct(MetaType, String),
}

impl std::error::Error for RafsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl Display for RafsError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum MetaType {
    Regular,
    Dir,
    Symlink,
}

/// Specialized version of std::result::Result<> for Rafs.
pub type RafsResult<T> = std::result::Result<T, RafsError>;

/// Handler to read file system bootstrap.
pub type RafsIoReader = Box<dyn RafsIoRead>;

/// A helper trait for RafsIoReader.
pub trait RafsIoRead: Read + AsRawFd + Seek + Send {}

impl RafsIoRead for File {}

/// Handler to write file system bootstrap.
pub type RafsIoWriter = Box<dyn RafsIoWrite>;

/// A helper trait for RafsIoWriter.
pub trait RafsIoWrite: Write + Seek + 'static {
    fn as_any(&self) -> &dyn Any;

    fn validate_alignment(&mut self, size: usize, alignment: usize) -> Result<usize> {
        if alignment != 0 {
            let cur = self.seek(SeekFrom::Current(0))?;

            if (size & (alignment - 1) != 0) || (cur & (alignment as u64 - 1) != 0) {
                return Err(einval!("unaligned data"));
            }
        }

        Ok(size)
    }

    /// write padding to align to RAFS_ALIGNMENT.
    fn write_padding(&mut self, size: usize) -> Result<()> {
        if size > WRITE_PADDING_DATA.len() {
            return Err(einval!("invalid padding size"));
        }
        self.write_all(&WRITE_PADDING_DATA[0..size])
    }

    /// Seek the writer to the end.
    fn seek_to_end(&mut self) -> Result<u64> {
        self.seek(SeekFrom::End(0)).map_err(|e| {
            error!("Seeking to end fails, {}", e);
            e
        })
    }

    /// Seek the writer to the `offset`.
    fn seek_offset(&mut self, offset: u64) -> Result<u64> {
        self.seek(SeekFrom::Start(offset)).map_err(|e| {
            error!("Seeking to offset {} from start fails, {}", offset, e);
            e
        })
    }

    /// Seek the writer to current position plus the specified offset.
    fn seek_current(&mut self, offset: i64) -> Result<u64> {
        self.seek(SeekFrom::Current(offset))
    }

    /// Do some finalization works.
    fn finalize(&mut self, _name: Option<String>) -> anyhow::Result<()> {
        Ok(())
    }

    /// Get all data has been written.
    fn data(&self) -> &[u8] {
        unimplemented!();
    }
}

impl RafsIoWrite for File {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Rust file I/O is un-buffered by default. If we have many small write calls
// to a file, should use BufWriter. BufWriter maintains an in-memory buffer
// for writing, minimizing the number of system calls required.
impl RafsIoWrite for BufWriter<File> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const WRITE_PADDING_DATA: [u8; 64] = [0u8; 64];

impl dyn RafsIoRead {
    /// Seek the reader to next aligned position.
    pub fn seek_to_next_aligned(&mut self, last_read_len: usize, alignment: usize) -> Result<u64> {
        let suffix = last_read_len & (alignment - 1);
        let offset = if suffix == 0 { 0 } else { alignment - suffix };

        self.seek(SeekFrom::Current(offset as i64)).map_err(|e| {
            error!("Seeking to offset {} from current fails, {}", offset, e);
            e
        })
    }

    /// Move the reader current position forward with `plus_offset` bytes.
    pub fn seek_plus_offset(&mut self, plus_offset: i64) -> Result<u64> {
        // Seek should not fail otherwise rafs goes insane.
        self.seek(SeekFrom::Current(plus_offset)).map_err(|e| {
            error!(
                "Seeking to offset {} from current fails, {}",
                plus_offset, e
            );
            e
        })
    }

    /// Seek the reader to the `offset`.
    pub fn seek_to_offset(&mut self, offset: u64) -> Result<u64> {
        self.seek(SeekFrom::Start(offset)).map_err(|e| {
            error!("Seeking to offset {} from start fails, {}", offset, e);
            e
        })
    }

    /// Seek the reader to the end.
    pub fn seek_to_end(&mut self, offset: i64) -> Result<u64> {
        self.seek(SeekFrom::End(offset)).map_err(|e| {
            error!("Seeking to end fails, {}", e);
            e
        })
    }

    /// Create a reader from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> RafsResult<RafsIoReader> {
        let f = File::open(&path).map_err(|e| {
            RafsError::ReadMetadata(e, path.as_ref().to_string_lossy().into_owned())
        })?;

        Ok(Box::new(f))
    }
}

///  Iterator to walk all inodes of a Rafs filesystem.
pub struct RafsIterator<'a> {
    _rs: &'a RafsSuper,
    cursor_stack: Vec<(Arc<dyn RafsInode>, PathBuf)>,
}

impl<'a> RafsIterator<'a> {
    /// Create a new iterator to visit a Rafs filesystem.
    pub fn new(rs: &'a RafsSuper) -> Self {
        let cursor_stack = match rs.get_inode(rs.superblock.root_ino(), false) {
            Ok(node) => {
                let path = PathBuf::from("/");
                vec![(node, path)]
            }
            Err(e) => {
                error!(
                    "failed to get root inode from the bootstrap {}, damaged or malicious file?",
                    e
                );
                vec![]
            }
        };

        RafsIterator {
            _rs: rs,
            cursor_stack,
        }
    }
}

impl<'a> Iterator for RafsIterator<'a> {
    type Item = (Arc<dyn RafsInode>, PathBuf);

    fn next(&mut self) -> Option<Self::Item> {
        let (node, path) = self.cursor_stack.pop()?;
        if node.is_dir() {
            let children = 0..node.get_child_count();
            for idx in children.rev() {
                if let Ok(child) = node.get_child_by_index(idx) {
                    let child_path = path.join(child.name());
                    self.cursor_stack.push((child, child_path));
                } else {
                    error!(
                        "failed to get child inode from the bootstrap, damaged or malicious file?"
                    );
                }
            }
        }
        Some((node, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::RafsMode;
    use std::fs::OpenOptions;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_rafs_io_writer() {
        let mut file = TempFile::new().unwrap().into_file();

        assert!(file.validate_alignment(2, 8).is_err());
        assert!(file.validate_alignment(7, 8).is_err());
        assert!(file.validate_alignment(9, 8).is_err());
        assert!(file.validate_alignment(8, 8).is_ok());

        file.write_all(&[0x0u8; 7]).unwrap();
        assert!(file.validate_alignment(8, 8).is_err());
        {
            let obj: &mut dyn RafsIoWrite = &mut file;
            obj.write_padding(1).unwrap();
        }
        assert!(file.validate_alignment(8, 8).is_ok());
        file.write_all(&[0x0u8; 1]).unwrap();
        assert!(file.validate_alignment(8, 8).is_err());

        let obj: &mut dyn RafsIoRead = &mut file;
        assert_eq!(obj.seek_to_offset(0).unwrap(), 0);
        assert_eq!(obj.seek_plus_offset(7).unwrap(), 7);
        assert_eq!(obj.seek_to_next_aligned(7, 8).unwrap(), 8);
        assert_eq!(obj.seek_plus_offset(7).unwrap(), 15);
    }

    #[test]
    fn test_rafs_iterator() {
        let root_dir = &std::env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let path = PathBuf::from(root_dir).join("../tests/texture/bootstrap/image_v2.boot");
        let bootstrap = OpenOptions::new()
            .read(true)
            .write(false)
            .open(&path)
            .unwrap();
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: false,
            ..Default::default()
        };
        rs.load(&mut (Box::new(bootstrap) as RafsIoReader)).unwrap();
        let iter = RafsIterator::new(&rs);

        let mut last = false;
        for (idx, (_node, path)) in iter.enumerate() {
            assert!(!last);
            if idx == 1 {
                assert_eq!(path, PathBuf::from("/bin"));
            } else if idx == 2 {
                assert_eq!(path, PathBuf::from("/boot"));
            } else if idx == 3 {
                assert_eq!(path, PathBuf::from("/dev"));
            } else if idx == 10 {
                assert_eq!(path, PathBuf::from("/etc/DIR_COLORS.256color"));
            } else if idx == 11 {
                assert_eq!(path, PathBuf::from("/etc/DIR_COLORS.lightbgcolor"));
            } else if path == PathBuf::from("/var/yp") {
                last = true;
            }
        }
        assert!(last);
    }
}
