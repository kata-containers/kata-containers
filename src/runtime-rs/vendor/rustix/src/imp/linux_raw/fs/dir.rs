use super::FileType;
use crate::as_ptr;
#[cfg(not(any(io_lifetimes_use_std, not(feature = "std"))))]
use crate::fd::IntoFd;
use crate::fd::{AsFd, BorrowedFd};
use crate::ffi::{ZStr, ZString};
use crate::io::{self, OwnedFd};
use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use linux_raw_sys::general::linux_dirent64;

/// `DIR*`
pub struct Dir {
    fd: OwnedFd,
    buf: Vec<u8>,
    pos: usize,
    next: Option<u64>,
}

impl Dir {
    /// Construct a `Dir`, assuming ownership of the file descriptor.
    #[cfg(not(any(io_lifetimes_use_std, not(feature = "std"))))]
    #[inline]
    pub fn from<F: IntoFd>(fd: F) -> io::Result<Self> {
        let fd = fd.into_fd();
        Self::_from(fd.into())
    }

    /// Construct a `Dir`, assuming ownership of the file descriptor.
    #[cfg(any(io_lifetimes_use_std, not(feature = "std")))]
    #[inline]
    pub fn from<F: Into<crate::fd::OwnedFd>>(fd: F) -> io::Result<Self> {
        let fd: crate::fd::OwnedFd = fd.into();
        let fd: OwnedFd = fd.into();
        Self::_from(fd)
    }

    #[inline]
    fn _from(fd: OwnedFd) -> io::Result<Self> {
        Ok(Self {
            fd,
            buf: Vec::new(),
            pos: 0,
            next: None,
        })
    }

    /// `rewinddir(self)`
    #[inline]
    pub fn rewind(&mut self) {
        self.pos = self.buf.len();
        self.next = Some(0);
    }

    /// `readdir(self)`, where `None` means the end of the directory.
    pub fn read(&mut self) -> Option<io::Result<DirEntry>> {
        if let Some(next) = self.next.take() {
            match crate::imp::fs::syscalls::_seek(
                self.fd.as_fd(),
                next as i64,
                linux_raw_sys::general::SEEK_SET,
            ) {
                Ok(_) => (),
                Err(err) => return Some(Err(err)),
            }
        }

        // Compute linux_dirent64 field offsets.
        let z = linux_dirent64 {
            d_ino: 0_u64,
            d_off: 0_i64,
            d_type: 0_u8,
            d_reclen: 0_u16,
            d_name: Default::default(),
        };
        let base = as_ptr(&z) as usize;
        let offsetof_d_reclen = (as_ptr(&z.d_reclen) as usize) - base;
        let offsetof_d_name = (as_ptr(&z.d_name) as usize) - base;
        let offsetof_d_ino = (as_ptr(&z.d_ino) as usize) - base;
        let offsetof_d_type = (as_ptr(&z.d_type) as usize) - base;

        // Test if we need more entries, and if so, read more.
        if self.buf.len() - self.pos < size_of::<linux_dirent64>() {
            match self.read_more()? {
                Ok(()) => (),
                Err(e) => return Some(Err(e)),
            }
        }

        // We successfully read an entry. Extract the fields.
        let pos = self.pos;

        // Do an unaligned u16 load.
        let d_reclen = u16::from_ne_bytes([
            self.buf[pos + offsetof_d_reclen],
            self.buf[pos + offsetof_d_reclen + 1],
        ]);
        assert!(self.buf.len() - pos >= d_reclen as usize);
        self.pos += d_reclen as usize;

        // Read the NUL-terminated name from the `d_name` field. Without
        // `unsafe`, we need to scan for the NUL twice: once to obtain a size
        // for the slice, and then once within `ZStr::from_bytes_with_nul`.
        let name_start = pos + offsetof_d_name;
        let name_len = self.buf[name_start..]
            .iter()
            .position(|x| *x == b'\0')
            .unwrap();
        let name =
            ZStr::from_bytes_with_nul(&self.buf[name_start..name_start + name_len + 1]).unwrap();
        let name = name.to_owned();
        assert!(name.as_bytes().len() <= self.buf.len() - name_start);

        // Do an unaligned u64 load.
        let d_ino = u64::from_ne_bytes([
            self.buf[pos + offsetof_d_ino],
            self.buf[pos + offsetof_d_ino + 1],
            self.buf[pos + offsetof_d_ino + 2],
            self.buf[pos + offsetof_d_ino + 3],
            self.buf[pos + offsetof_d_ino + 4],
            self.buf[pos + offsetof_d_ino + 5],
            self.buf[pos + offsetof_d_ino + 6],
            self.buf[pos + offsetof_d_ino + 7],
        ]);

        let d_type = self.buf[pos + offsetof_d_type];

        // Check that our types correspond to the `linux_dirent64` types.
        let _ = linux_dirent64 {
            d_ino,
            d_off: 0,
            d_type,
            d_reclen,
            d_name: Default::default(),
        };

        Some(Ok(DirEntry {
            d_ino,
            d_type,
            name,
        }))
    }

    fn read_more(&mut self) -> Option<io::Result<()>> {
        // Capacity increment currently chosen by wild guess.
        self.buf
            .resize(self.buf.capacity() + 32 * size_of::<linux_dirent64>(), 0);
        self.pos = 0;
        let nread = match crate::imp::fs::syscalls::getdents(self.fd.as_fd(), &mut self.buf) {
            Ok(nread) => nread,
            Err(err) => return Some(Err(err)),
        };
        self.buf.resize(nread, 0);
        if nread == 0 {
            None
        } else {
            Some(Ok(()))
        }
    }
}

impl AsFd for Dir {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Iterator for Dir {
    type Item = io::Result<DirEntry>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Self::read(self)
    }
}

impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dir").field("fd", &self.fd).finish()
    }
}

/// `struct dirent`
#[derive(Debug)]
pub struct DirEntry {
    d_ino: u64,
    d_type: u8,
    name: ZString,
}

impl DirEntry {
    /// Returns the file name of this directory entry.
    #[inline]
    pub fn file_name(&self) -> &ZStr {
        &self.name
    }

    /// Returns the type of this directory entry.
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType::from_dirent_d_type(self.d_type)
    }

    /// Return the inode number of this directory entry.
    #[inline]
    pub fn ino(&self) -> u64 {
        self.d_ino
    }
}
