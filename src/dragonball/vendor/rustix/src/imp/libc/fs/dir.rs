use super::super::c;
use super::super::conv::owned_fd;
#[cfg(not(target_os = "illumos"))]
use super::FileType;
#[cfg(not(any(io_lifetimes_use_std, not(feature = "std"))))]
use crate::fd::IntoFd;
use crate::fd::{AsFd, BorrowedFd, RawFd};
use crate::ffi::ZStr;
#[cfg(target_os = "wasi")]
use crate::ffi::ZString;
use crate::io::{self, OwnedFd};
#[cfg(target_os = "wasi")]
use alloc::borrow::ToOwned;
#[cfg(not(any(
    target_os = "android",
    target_os = "emscripten",
    target_os = "l4re",
    target_os = "linux",
    target_os = "openbsd",
)))]
use c::dirent as libc_dirent;
#[cfg(not(any(
    target_os = "android",
    target_os = "emscripten",
    target_os = "l4re",
    target_os = "linux",
)))]
use c::readdir as libc_readdir;
#[cfg(any(
    target_os = "android",
    target_os = "emscripten",
    target_os = "l4re",
    target_os = "linux"
))]
use c::{dirent64 as libc_dirent, readdir64 as libc_readdir};
use core::fmt;
use core::mem::zeroed;
use core::ptr::NonNull;
use errno::{errno, set_errno, Errno};

/// `DIR*`
#[repr(transparent)]
pub struct Dir(NonNull<c::DIR>);

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

    fn _from(fd: OwnedFd) -> io::Result<Self> {
        let raw = owned_fd(fd);
        unsafe {
            let d = c::fdopendir(raw);
            if let Some(d) = NonNull::new(d) {
                Ok(Self(d))
            } else {
                let e = io::Error::last_os_error();
                let _ = c::close(raw);
                Err(e)
            }
        }
    }

    /// `rewinddir(self)`
    #[inline]
    pub fn rewind(&mut self) {
        unsafe { c::rewinddir(self.0.as_ptr()) }
    }

    /// `readdir(self)`, where `None` means the end of the directory.
    pub fn read(&mut self) -> Option<io::Result<DirEntry>> {
        set_errno(Errno(0));
        let dirent_ptr = unsafe { libc_readdir(self.0.as_ptr()) };
        if dirent_ptr.is_null() {
            let curr_errno = errno().0;
            if curr_errno == 0 {
                // We successfully reached the end of the stream.
                None
            } else {
                // `errno` is unknown or non-zero, so an error occurred.
                Some(Err(io::Error(curr_errno)))
            }
        } else {
            // We successfully read an entry.
            unsafe {
                // We have our own copy of OpenBSD's dirent; check that the
                // layout minimally matches libc's.
                #[cfg(target_os = "openbsd")]
                check_dirent_layout(&*dirent_ptr);

                let result = DirEntry {
                    dirent: read_dirent(core::mem::transmute(&*dirent_ptr)),

                    #[cfg(target_os = "wasi")]
                    name: ZStr::from_ptr((*dirent_ptr).d_name.as_ptr().cast()).to_owned(),
                };

                Some(Ok(result))
            }
        }
    }
}

// A `dirent` pointer returned from `readdir` may not point to a full `dirent`
// struct, as the name is NUL-terminated and memory may not be allocated for
// the full extent of the struct. Copy the fields one at a time.
unsafe fn read_dirent(input: &libc_dirent) -> libc_dirent {
    #[cfg(not(target_os = "illumos"))]
    let d_type = input.d_type;

    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "wasi",
    )))]
    let d_off = input.d_off;

    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    let d_ino = input.d_ino;

    #[cfg(any(target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
    let d_fileno = input.d_fileno;

    #[cfg(not(target_os = "wasi"))]
    let d_reclen = input.d_reclen;

    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "ios",
        target_os = "macos"
    ))]
    let d_namlen = input.d_namlen;

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let d_seekoff = input.d_seekoff;

    // Construct the input. Rust will give us an error if any OS has a input
    // with a field that we missed here. And we can avoid blindly copying the
    // whole `d_name` field, which may not be entirely allocated.
    #[cfg_attr(target_os = "wasi", allow(unused_mut))]
    let mut dirent = libc_dirent {
        #[cfg(not(target_os = "illumos"))]
        d_type,
        #[cfg(not(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "wasi",
        )))]
        d_off,
        #[cfg(not(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        d_ino,
        #[cfg(any(target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
        d_fileno,
        #[cfg(not(target_os = "wasi"))]
        d_reclen,
        #[cfg(any(
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        d_namlen,
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        d_seekoff,
        // The `d_name` field is NUL-terminated, and we need to be careful not
        // to read bytes past the NUL, even though they're within the nominal
        // extent of the `struct dirent`, because they may not be allocated. So
        // don't read it from `dirent_ptr`.
        //
        // In theory this could use `MaybeUninit::uninit().assume_init()`, but
        // that [invokes undefined behavior].
        //
        // [invokes undefined behavior]: https://doc.rust-lang.org/stable/core/mem/union.MaybeUninit.html#initialization-invariant
        d_name: zeroed(),
        #[cfg(target_os = "openbsd")]
        __d_padding: zeroed(),
    };

    // Copy from d_name, reading up to and including the first NUL.
    #[cfg(not(target_os = "wasi"))]
    {
        let name_len = ZStr::from_ptr(input.d_name.as_ptr().cast())
            .to_bytes()
            .len()
            + 1;
        dirent.d_name[..name_len].copy_from_slice(&input.d_name[..name_len]);
    }

    dirent
}

/// `Dir` implements `Send` but not `Sync`, because we use `readdir` which is
/// not guaranteed to be thread-safe. Users can wrap this in a `Mutex` if they
/// need `Sync`, which is effectively what'd need to do to implement `Sync`
/// ourselves.
unsafe impl Send for Dir {}

impl AsFd for Dir {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(c::dirfd(self.0.as_ptr()) as RawFd) }
    }
}

impl Drop for Dir {
    #[inline]
    fn drop(&mut self) {
        unsafe { c::closedir(self.0.as_ptr()) };
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
        f.debug_struct("Dir")
            .field("fd", unsafe { &c::dirfd(self.0.as_ptr()) })
            .finish()
    }
}

/// `struct dirent`
#[derive(Debug)]
pub struct DirEntry {
    dirent: libc_dirent,

    #[cfg(target_os = "wasi")]
    name: ZString,
}

impl DirEntry {
    /// Returns the file name of this directory entry.
    #[inline]
    pub fn file_name(&self) -> &ZStr {
        #[cfg(not(target_os = "wasi"))]
        unsafe {
            ZStr::from_ptr(self.dirent.d_name.as_ptr().cast())
        }

        #[cfg(target_os = "wasi")]
        &self.name
    }

    /// Returns the type of this directory entry.
    #[inline]
    #[cfg(not(target_os = "illumos"))]
    pub fn file_type(&self) -> FileType {
        FileType::from_dirent_d_type(self.dirent.d_type)
    }

    /// Return the inode number of this directory entry.
    #[cfg(not(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    #[inline]
    pub fn ino(&self) -> u64 {
        self.dirent.d_ino
    }

    /// Return the inode number of this directory entry.
    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[inline]
    pub fn ino(&self) -> u64 {
        #[allow(clippy::useless_conversion)]
        self.dirent.d_fileno.into()
    }
}

/// libc's OpenBSD `dirent` has a private field so we can't construct it
/// directly, so we declare it ourselves to make all fields accessible.
#[cfg(target_os = "openbsd")]
#[repr(C)]
#[derive(Debug)]
struct libc_dirent {
    d_fileno: c::ino_t,
    d_off: c::off_t,
    d_reclen: u16,
    d_type: u8,
    d_namlen: u8,
    __d_padding: [u8; 4],
    d_name: [c::c_char; 256],
}

/// We have our own copy of OpenBSD's dirent; check that the layout
/// minimally matches libc's.
#[cfg(target_os = "openbsd")]
fn check_dirent_layout(dirent: &c::dirent) {
    use crate::as_ptr;
    use core::mem::{align_of, size_of};

    // Check that the basic layouts match.
    assert_eq!(size_of::<libc_dirent>(), size_of::<c::dirent>());
    assert_eq!(align_of::<libc_dirent>(), align_of::<c::dirent>());

    // Check that the field offsets match.
    assert_eq!(
        {
            let z = libc_dirent {
                d_fileno: 0_u64,
                d_off: 0_i64,
                d_reclen: 0_u16,
                d_type: 0_u8,
                d_namlen: 0_u8,
                __d_padding: [0_u8; 4],
                d_name: [0 as c::c_char; 256],
            };
            let base = as_ptr(&z) as usize;
            (
                (as_ptr(&z.d_fileno) as usize) - base,
                (as_ptr(&z.d_off) as usize) - base,
                (as_ptr(&z.d_reclen) as usize) - base,
                (as_ptr(&z.d_type) as usize) - base,
                (as_ptr(&z.d_namlen) as usize) - base,
                (as_ptr(&z.d_name) as usize) - base,
            )
        },
        {
            let z = dirent;
            let base = as_ptr(z) as usize;
            (
                (as_ptr(&z.d_fileno) as usize) - base,
                (as_ptr(&z.d_off) as usize) - base,
                (as_ptr(&z.d_reclen) as usize) - base,
                (as_ptr(&z.d_type) as usize) - base,
                (as_ptr(&z.d_namlen) as usize) - base,
                (as_ptr(&z.d_name) as usize) - base,
            )
        }
    );
}
