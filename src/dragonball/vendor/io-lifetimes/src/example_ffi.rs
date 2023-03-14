//! This is just a sample of what FFI using this crate can look like.

#![cfg_attr(not(rustc_attrs), allow(unused_imports))]
#![allow(missing_docs)]

#[cfg(any(unix, target_os = "wasi"))]
use crate::{BorrowedFd, OwnedFd};
#[cfg(windows)]
use crate::{BorrowedHandle, HandleOrInvalid, OwnedHandle};

#[cfg(any(unix, target_os = "wasi"))]
use libc::{c_char, c_int, c_void, size_t, ssize_t};
#[cfg(windows)]
use winapi::{
    shared::minwindef::{BOOL, DWORD, LPCVOID, LPDWORD, LPVOID},
    shared::ntdef::{HANDLE, LPCWSTR},
    um::minwinbase::{LPOVERLAPPED, LPSECURITY_ATTRIBUTES},
};

// Declare a few FFI functions ourselves, to show off the FFI ergonomics.
#[cfg(all(rustc_attrs, any(unix, target_os = "wasi")))]
extern "C" {
    pub fn open(pathname: *const c_char, flags: c_int, ...) -> Option<OwnedFd>;
}
#[cfg(any(unix, target_os = "wasi"))]
extern "C" {
    pub fn read(fd: BorrowedFd<'_>, ptr: *mut c_void, size: size_t) -> ssize_t;
    pub fn write(fd: BorrowedFd<'_>, ptr: *const c_void, size: size_t) -> ssize_t;
}
#[cfg(any(unix, target_os = "wasi"))]
pub use libc::{O_CLOEXEC, O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY};

/// The Windows analogs of the above. Note the use of [`HandleOrInvalid`] as
/// the return type for `CreateFileW`, since that function is defined to return
/// [`INVALID_HANDLE_VALUE`] on error instead of null.
#[cfg(windows)]
extern "system" {
    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HandleOrInvalid;
    pub fn ReadFile(
        hFile: BorrowedHandle<'_>,
        lpBuffer: LPVOID,
        nNumberOfBytesToRead: DWORD,
        lpNumberOfBytesRead: LPDWORD,
        lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn WriteFile(
        hFile: BorrowedHandle<'_>,
        lpBuffer: LPCVOID,
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: LPDWORD,
        lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
}
#[cfg(windows)]
pub use winapi::{
    shared::minwindef::{FALSE, TRUE},
    um::fileapi::{CREATE_ALWAYS, CREATE_NEW, OPEN_EXISTING},
    um::winnt::{FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE},
};
