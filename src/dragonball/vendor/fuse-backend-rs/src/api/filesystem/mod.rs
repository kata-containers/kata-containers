// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! Structs and Traits for filesystem server to implement a concrete Fuse filesystem.
//!
//! The [FileSystem](trait.FileSystem.html) trait is the connection between the transport layer
//! and the backend filesystem server. Other structs are used to pass information from the

use std::convert::TryInto;
use std::io;
use std::time::Duration;

use crate::abi::fuse_abi as fuse;
use crate::file_traits::FileReadWriteVolatile;

pub use fuse::FsOptions;
pub use fuse::OpenOptions;
pub use fuse::SetattrValid;
pub use fuse::ROOT_ID;

use crate::abi::fuse_abi::{ino64_t, stat64};

#[cfg(feature = "async-io")]
mod async_io;
#[cfg(feature = "async-io")]
pub use async_io::{AsyncFileSystem, AsyncZeroCopyReader, AsyncZeroCopyWriter};

mod sync_io;
pub use sync_io::FileSystem;

/// Information about a path in the filesystem.
#[derive(Copy, Clone)]
pub struct Entry {
    /// An `Inode` that uniquely identifies this path. During `lookup`, setting this to `0` means a
    /// negative entry. Returning `ENOENT` also means a negative entry but setting this to `0`
    /// allows the kernel to cache the negative result for `entry_timeout`. The value should be
    /// produced by converting a `FileSystem::Inode` into a `u64`.
    pub inode: u64,

    /// The generation number for this `Entry`. Typically used for network file systems. An `inode`
    /// / `generation` pair must be unique over the lifetime of the file system (rather than just
    /// the lifetime of the mount). In other words, if a `FileSystem` implementation re-uses an
    /// `Inode` after it has been deleted then it must assign a new, previously unused generation
    /// number to the `Inode` at the same time.
    pub generation: u64,

    /// Inode attributes. Even if `attr_timeout` is zero, `attr` must be correct. For example, for
    /// `open()`, FUSE uses `attr.st_size` from `lookup()` to determine how many bytes to request.
    /// If this value is not correct, incorrect data will be returned.
    pub attr: stat64,

    /// Flags for 'fuse::Attr.flags'
    pub attr_flags: u32,

    /// How long the values in `attr` should be considered valid. If the attributes of the `Entry`
    /// are only modified by the FUSE client, then this should be set to a very large value.
    pub attr_timeout: Duration,

    /// How long the name associated with this `Entry` should be considered valid. If directory
    /// entries are only changed or deleted by the FUSE client, then this should be set to a very
    /// large value.
    pub entry_timeout: Duration,
}

impl From<Entry> for fuse::EntryOut {
    fn from(entry: Entry) -> fuse::EntryOut {
        fuse::EntryOut {
            nodeid: entry.inode,
            generation: entry.generation,
            entry_valid: entry.entry_timeout.as_secs(),
            attr_valid: entry.attr_timeout.as_secs(),
            entry_valid_nsec: entry.entry_timeout.subsec_nanos(),
            attr_valid_nsec: entry.attr_timeout.subsec_nanos(),
            attr: fuse::Attr::with_flags(entry.attr, entry.attr_flags),
        }
    }
}

impl Default for Entry {
    fn default() -> Self {
        Entry {
            inode: 0,
            generation: 0,
            attr: unsafe { std::mem::zeroed() },
            attr_flags: 0,
            attr_timeout: Duration::default(),
            entry_timeout: Duration::default(),
        }
    }
}

/// Represents information about an entry in a directory.
#[derive(Copy, Clone)]
pub struct DirEntry<'a> {
    /// The inode number for this entry. This does NOT have to be the same as the `Inode` for this
    /// directory entry. However, it must be the same as the `attr.st_ino` field of the `Entry` that
    /// would be returned by a `lookup` request in the parent directory for `name`.
    pub ino: ino64_t,

    /// Any non-zero value that the kernel can use to identify the current point in the directory
    /// entry stream. It does not need to be the actual physical position. A value of `0` is
    /// reserved to mean "from the beginning" and should never be used. The `offset` value of the
    /// first entry in a stream should point to the beginning of the second entry and so on.
    pub offset: u64,

    /// The type of this directory entry. Valid values are any of the `libc::DT_*` constants.
    pub type_: u32,

    /// The name of this directory entry. There are no requirements for the contents of this field
    /// and any sequence of bytes is considered valid.
    pub name: &'a [u8],
}

/// Represents a fuse lock
#[derive(Copy, Clone)]
pub struct FileLock {
    /// Lock range start
    pub start: u64,
    /// Lock range end, exclusive?
    pub end: u64,
    /// Lock type
    pub lock_type: u32,
    /// thread id who owns the lock
    pub pid: u32,
}

impl From<fuse::FileLock> for FileLock {
    fn from(l: fuse::FileLock) -> FileLock {
        FileLock {
            start: l.start,
            end: l.end,
            lock_type: l.type_,
            pid: l.pid,
        }
    }
}

impl From<FileLock> for fuse::FileLock {
    fn from(l: FileLock) -> fuse::FileLock {
        fuse::FileLock {
            start: l.start,
            end: l.end,
            type_: l.lock_type,
            pid: l.pid,
        }
    }
}

/// ioctl data and result
#[derive(Default, Clone)]
pub struct IoctlData<'a> {
    /// ioctl result
    pub result: i32,
    /// ioctl data
    pub data: Option<&'a [u8]>,
}

/// A reply to a `getxattr` method call.
pub enum GetxattrReply {
    /// The value of the requested extended attribute. This can be arbitrary textual or binary data
    /// and does not need to be nul-terminated.
    Value(Vec<u8>),

    /// The size of the buffer needed to hold the value of the requested extended attribute. Should
    /// be returned when the `size` parameter is 0. Callers should note that it is still possible
    /// for the size of the value to change in between `getxattr` calls and should not assume that a
    /// subsequent call to `getxattr` with the returned count will always succeed.
    Count(u32),
}

/// A reply to a `listxattr` method call.
pub enum ListxattrReply {
    /// A buffer containing a nul-separated list of the names of all the extended attributes
    /// associated with this `Inode`. This list of names may be unordered and includes a namespace
    /// prefix. There may be several disjoint namespaces associated with a single `Inode`.
    Names(Vec<u8>),

    /// This size of the buffer needed to hold the full list of extended attribute names associated
    /// with this `Inode`. Should be returned when the `size` parameter is 0. Callers should note
    /// that it is still possible for the set of extended attributes to change between `listxattr`
    /// calls and so should not assume that a subsequent call to `listxattr` with the returned count
    /// will always succeed.
    Count(u32),
}

/// A trait for directly copying data from the fuse transport into a `File` without first storing it
/// in an intermediate buffer.
pub trait ZeroCopyReader: io::Read {
    /// Copies at most `count` bytes from `self` directly into `f` at offset `off` without storing
    /// it in any intermediate buffers. If the return value is `Ok(n)` then it must be guaranteed
    /// that `0 <= n <= count`. If `n` is `0`, then it can indicate one of 3 possibilities:
    ///
    /// 1. There is no more data left in `self`.
    /// 2. There is no more space in `f`.
    /// 3. `count` was `0`.
    ///
    /// # Errors
    ///
    /// If any error is returned then the implementation must guarantee that no bytes were copied
    /// from `self`. If the underlying write to `f` returns `0` then the implementation must return
    /// an error of the kind `io::ErrorKind::WriteZero`.
    fn read_to(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize>;

    /// Copies exactly `count` bytes of data from `self` into `f` at offset `off`. `off + count`
    /// must be less than `u64::MAX`.
    ///
    /// # Errors
    ///
    /// If an error is returned then the number of bytes copied from `self` is unspecified but it
    /// will never be more than `count`.
    fn read_exact_to(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        mut count: usize,
        mut off: u64,
    ) -> io::Result<()> {
        let c = count
            .try_into()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if off.checked_add(c).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "`off` + `count` must be less than u64::MAX",
            ));
        }

        while count > 0 {
            match self.read_to(f, count, off) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to fill whole buffer",
                    ))
                }
                Ok(n) => {
                    count -= n;
                    off += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Copies all remaining bytes from `self` into `f` at offset `off`. Equivalent to repeatedly
    /// calling `read_to` until it returns either `Ok(0)` or a non-`ErrorKind::Interrupted` error.
    ///
    /// # Errors
    ///
    /// If an error is returned then the number of bytes copied from `self` is unspecified.
    fn copy_to_end(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        mut off: u64,
    ) -> io::Result<usize> {
        let mut out = 0;
        loop {
            match self.read_to(f, ::std::usize::MAX, off) {
                Ok(0) => return Ok(out),
                Ok(n) => {
                    off = off.saturating_add(n as u64);
                    out += n;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

/// A trait for directly copying data from a `File` into the fuse transport without first storing
/// it in an intermediate buffer.
pub trait ZeroCopyWriter: io::Write {
    /// Copies at most `count` bytes from `f` at offset `off` directly into `self` without storing
    /// it in any intermediate buffers. If the return value is `Ok(n)` then it must be guaranteed
    /// that `0 <= n <= count`. If `n` is `0`, then it can indicate one of 3 possibilities:
    ///
    /// 1. There is no more data left in `f`.
    /// 2. There is no more space in `self`.
    /// 3. `count` was `0`.
    ///
    /// # Errors
    ///
    /// If any error is returned then the implementation must guarantee that no bytes were copied
    /// from `f`. If the underlying read from `f` returns `0` then the implementation must return an
    /// error of the kind `io::ErrorKind::UnexpectedEof`.
    fn write_from(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize>;

    /// Copies exactly `count` bytes of data from `f` at offset `off` into `self`. `off + count`
    /// must be less than `u64::MAX`.
    ///
    /// # Errors
    ///
    /// If an error is returned then the number of bytes copied from `self` is unspecified but it
    /// well never be more than `count`.
    fn write_all_from(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        mut count: usize,
        mut off: u64,
    ) -> io::Result<()> {
        let c = count
            .try_into()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if off.checked_add(c).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "`off` + `count` must be less than u64::MAX",
            ));
        }

        while count > 0 {
            match self.write_from(f, count, off) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => {
                    // No need for checked math here because we verified that `off + count` will not
                    // overflow and `n` must be <= `count`.
                    count -= n;
                    off += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Copies all remaining bytes from `f` at offset `off` into `self`. Equivalent to repeatedly
    /// calling `write_from` until it returns either `Ok(0)` or a non-`ErrorKind::Interrupted`
    /// error.
    ///
    /// # Errors
    ///
    /// If an error is returned then the number of bytes copied from `f` is unspecified.
    fn copy_to_end(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        mut off: u64,
    ) -> io::Result<usize> {
        let mut out = 0;
        loop {
            match self.write_from(f, ::std::usize::MAX, off) {
                Ok(0) => return Ok(out),
                Ok(n) => {
                    off = off.saturating_add(n as u64);
                    out += n;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

/// Additional context associated with requests.
#[derive(Default, Clone, Copy, Debug)]
pub struct Context {
    /// The user ID of the calling process.
    pub uid: libc::uid_t,

    /// The group ID of the calling process.
    pub gid: libc::gid_t,

    /// The thread group ID of the calling process.
    pub pid: libc::pid_t,
}

impl Context {
    /// Create a new 'Context' object.
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<&fuse::InHeader> for Context {
    fn from(source: &fuse::InHeader) -> Self {
        Context {
            uid: source.uid,
            gid: source.gid,
            pid: source.pid as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::fuse_abi::Attr;

    #[test]
    fn test_from_fuse_header() {
        let fuse_header = &fuse::InHeader {
            len: 16,
            opcode: 0,
            unique: 1,
            nodeid: 2,
            uid: 3,
            gid: 4,
            pid: 5,
            padding: 0,
        };
        let header: Context = fuse_header.into();

        assert_eq!(header.uid, 3);
        assert_eq!(header.gid, 4);
        assert_eq!(header.pid, 5);
    }

    #[test]
    fn test_into_fuse_entry() {
        let attr = Attr {
            ..Default::default()
        };
        let entry = Entry {
            inode: 1,
            generation: 2,
            attr: attr.into(),
            ..Default::default()
        };
        let fuse_entry: fuse::EntryOut = entry.into();

        assert_eq!(fuse_entry.nodeid, 1);
        assert_eq!(fuse_entry.generation, 2);
    }
}
