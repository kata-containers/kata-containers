//! Vectored I/O

use crate::Result;
use crate::errno::Errno;
use libc::{self, c_int, c_void, size_t, off_t};
use std::io::{IoSlice, IoSliceMut};
use std::marker::PhantomData;
use std::os::unix::io::RawFd;

/// Low-level vectored write to a raw file descriptor
///
/// See also [writev(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/writev.html)
pub fn writev(fd: RawFd, iov: &[IoSlice<'_>]) -> Result<usize> {
    // SAFETY: to quote the documentation for `IoSlice`:
    // 
    // [IoSlice] is semantically a wrapper around a &[u8], but is 
    // guaranteed to be ABI compatible with the iovec type on Unix
    // platforms.
    //
    // Because it is ABI compatible, a pointer cast here is valid
    let res = unsafe { libc::writev(fd, iov.as_ptr() as *const libc::iovec, iov.len() as c_int) };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level vectored read from a raw file descriptor
///
/// See also [readv(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/readv.html)
pub fn readv(fd: RawFd, iov: &mut [IoSliceMut<'_>]) -> Result<usize> {
    // SAFETY: same as in writev(), IoSliceMut is ABI-compatible with iovec
    let res = unsafe { libc::readv(fd, iov.as_ptr() as *const libc::iovec, iov.len() as c_int) };

    Errno::result(res).map(|r| r as usize)
}

/// Write to `fd` at `offset` from buffers in `iov`.
///
/// Buffers in `iov` will be written in order until all buffers have been written
/// or an error occurs. The file offset is not changed.
///
/// See also: [`writev`](fn.writev.html) and [`pwrite`](fn.pwrite.html)
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn pwritev(fd: RawFd, iov: &[IoSlice<'_>],
               offset: off_t) -> Result<usize> {

    #[cfg(target_env = "uclibc")]
    let offset = offset as libc::off64_t; // uclibc doesn't use off_t

    // SAFETY: same as in writev()
    let res = unsafe {
        libc::pwritev(fd, iov.as_ptr() as *const libc::iovec, iov.len() as c_int, offset)
    };

    Errno::result(res).map(|r| r as usize)
}

/// Read from `fd` at `offset` filling buffers in `iov`.
///
/// Buffers in `iov` will be filled in order until all buffers have been filled,
/// no more bytes are available, or an error occurs. The file offset is not
/// changed.
///
/// See also: [`readv`](fn.readv.html) and [`pread`](fn.pread.html)
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn preadv(fd: RawFd, iov: &mut [IoSliceMut<'_>],
              offset: off_t) -> Result<usize> {
    #[cfg(target_env = "uclibc")]
    let offset = offset as libc::off64_t; // uclibc doesn't use off_t

    // SAFETY: same as in readv()
    let res = unsafe {
        libc::preadv(fd, iov.as_ptr() as *const libc::iovec, iov.len() as c_int, offset)
    };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level write to a file, with specified offset.
///
/// See also [pwrite(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/pwrite.html)
// TODO: move to unistd
pub fn pwrite(fd: RawFd, buf: &[u8], offset: off_t) -> Result<usize> {
    let res = unsafe {
        libc::pwrite(fd, buf.as_ptr() as *const c_void, buf.len() as size_t,
                    offset)
    };

    Errno::result(res).map(|r| r as usize)
}

/// Low-level read from a file, with specified offset.
///
/// See also [pread(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/pread.html)
// TODO: move to unistd
pub fn pread(fd: RawFd, buf: &mut [u8], offset: off_t) -> Result<usize>{
    let res = unsafe {
        libc::pread(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as size_t,
                   offset)
    };

    Errno::result(res).map(|r| r as usize)
}

/// A slice of memory in a remote process, starting at address `base`
/// and consisting of `len` bytes.
///
/// This is the same underlying C structure as `IoSlice`,
/// except that it refers to memory in some other process, and is
/// therefore not represented in Rust by an actual slice as `IoSlice` is. It
/// is used with [`process_vm_readv`](fn.process_vm_readv.html)
/// and [`process_vm_writev`](fn.process_vm_writev.html).
#[cfg(any(target_os = "linux", target_os = "android"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RemoteIoVec {
    /// The starting address of this slice (`iov_base`).
    pub base: usize,
    /// The number of bytes in this slice (`iov_len`).
    pub len: usize,
}

/// A vector of buffers.
///
/// Vectored I/O methods like [`writev`] and [`readv`] use this structure for
/// both reading and writing.  Each `IoVec` specifies the base address and
/// length of an area in memory.
#[deprecated(
    since = "0.24.0",
    note = "`IoVec` is no longer used in the public interface, use `IoSlice` or `IoSliceMut` instead"
)]
#[repr(transparent)]
#[allow(renamed_and_removed_lints)]
#[allow(clippy::unknown_clippy_lints)]
// Clippy false positive: https://github.com/rust-lang/rust-clippy/issues/8867
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IoVec<T>(pub(crate) libc::iovec, PhantomData<T>);

#[allow(deprecated)]
impl<T> IoVec<T> {
    /// View the `IoVec` as a Rust slice.
    #[deprecated(
        since = "0.24.0",
        note = "Use the `Deref` impl of `IoSlice` or `IoSliceMut` instead"
    )]
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        use std::slice;

        unsafe {
            slice::from_raw_parts(
                self.0.iov_base as *const u8,
                self.0.iov_len)
        }
    }
}

#[allow(deprecated)]
impl<'a> IoVec<&'a [u8]> {
    /// Create an `IoVec` from a Rust slice.
    #[deprecated(
        since = "0.24.0",
        note = "Use `IoSlice::new` instead"
    )]
    pub fn from_slice(buf: &'a [u8]) -> IoVec<&'a [u8]> {
        IoVec(libc::iovec {
            iov_base: buf.as_ptr() as *mut c_void,
            iov_len: buf.len() as size_t,
        }, PhantomData)
    }
}

#[allow(deprecated)]
impl<'a> IoVec<&'a mut [u8]> {
    /// Create an `IoVec` from a mutable Rust slice.
    #[deprecated(
        since = "0.24.0",
        note = "Use `IoSliceMut::new` instead"
    )]
    pub fn from_mut_slice(buf: &'a mut [u8]) -> IoVec<&'a mut [u8]> {
        IoVec(libc::iovec {
            iov_base: buf.as_ptr() as *mut c_void,
            iov_len: buf.len() as size_t,
        }, PhantomData)
    }
}

// The only reason IoVec isn't automatically Send+Sync is because libc::iovec
// contains raw pointers.
#[allow(deprecated)]
unsafe impl<T> Send for IoVec<T> where T: Send {}
#[allow(deprecated)]
unsafe impl<T> Sync for IoVec<T> where T: Sync {}

feature! {
#![feature = "process"]

/// Write data directly to another process's virtual memory
/// (see [`process_vm_writev`(2)]).
///
/// `local_iov` is a list of [`IoSlice`]s containing the data to be written,
/// and `remote_iov` is a list of [`RemoteIoVec`]s identifying where the
/// data should be written in the target process. On success, returns the
/// number of bytes written, which will always be a whole
/// number of `remote_iov` chunks.
///
/// This requires the same permissions as debugging the process using
/// [ptrace]: you must either be a privileged process (with
/// `CAP_SYS_PTRACE`), or you must be running as the same user as the
/// target process and the OS must have unprivileged debugging enabled.
///
/// This function is only available on Linux and Android(SDK23+).
///
/// [`process_vm_writev`(2)]: https://man7.org/linux/man-pages/man2/process_vm_writev.2.html
/// [ptrace]: ../ptrace/index.html
/// [`IoSlice`]: https://doc.rust-lang.org/std/io/struct.IoSlice.html
/// [`RemoteIoVec`]: struct.RemoteIoVec.html
#[cfg(all(any(target_os = "linux", target_os = "android"), not(target_env = "uclibc")))]
pub fn process_vm_writev(
    pid: crate::unistd::Pid,
    local_iov: &[IoSlice<'_>],
    remote_iov: &[RemoteIoVec]) -> Result<usize>
{
    let res = unsafe {
        libc::process_vm_writev(pid.into(),
                                local_iov.as_ptr() as *const libc::iovec, local_iov.len() as libc::c_ulong,
                                remote_iov.as_ptr() as *const libc::iovec, remote_iov.len() as libc::c_ulong, 0)
    };

    Errno::result(res).map(|r| r as usize)
}

/// Read data directly from another process's virtual memory
/// (see [`process_vm_readv`(2)]).
///
/// `local_iov` is a list of [`IoSliceMut`]s containing the buffer to copy
/// data into, and `remote_iov` is a list of [`RemoteIoVec`]s identifying
/// where the source data is in the target process. On success,
/// returns the number of bytes written, which will always be a whole
/// number of `remote_iov` chunks.
///
/// This requires the same permissions as debugging the process using
/// [`ptrace`]: you must either be a privileged process (with
/// `CAP_SYS_PTRACE`), or you must be running as the same user as the
/// target process and the OS must have unprivileged debugging enabled.
///
/// This function is only available on Linux and Android(SDK23+).
///
/// [`process_vm_readv`(2)]: https://man7.org/linux/man-pages/man2/process_vm_readv.2.html
/// [`ptrace`]: ../ptrace/index.html
/// [`IoSliceMut`]: https://doc.rust-lang.org/std/io/struct.IoSliceMut.html
/// [`RemoteIoVec`]: struct.RemoteIoVec.html
#[cfg(all(any(target_os = "linux", target_os = "android"), not(target_env = "uclibc")))]
pub fn process_vm_readv(
    pid: crate::unistd::Pid,
    local_iov: &mut [IoSliceMut<'_>],
    remote_iov: &[RemoteIoVec]) -> Result<usize>
{
    let res = unsafe {
        libc::process_vm_readv(pid.into(),
                               local_iov.as_ptr() as *const libc::iovec, local_iov.len() as libc::c_ulong,
                               remote_iov.as_ptr() as *const libc::iovec, remote_iov.len() as libc::c_ulong, 0)
    };

    Errno::result(res).map(|r| r as usize)
}
}
