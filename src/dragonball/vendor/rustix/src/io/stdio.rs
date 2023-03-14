//! Functions returning the stdio file descriptors.
//!
//! # Safety
//!
//! These access the file descriptors by absolute index value, and nothing
//! prevents them from being closed and reused. They should only be used in
//! `main` or other situations where one is in control of the process'
//! stdio streams.
#![allow(unsafe_code)]

use crate::imp;
use crate::io::OwnedFd;
use imp::fd::{BorrowedFd, FromRawFd, RawFd};

/// `STDIN_FILENO`—Standard input, borrowed.
///
/// # Safety
///
/// This function must be called from code which knows how the process'
/// standard input is being used. Often, this will be the `main` function or
/// code that knows its relationship with the `main` function.
///
/// The stdin file descriptor can be closed, potentially on other threads, in
/// which case the file descriptor index value could be dynamically reused for
/// other purposes, potentially on different threads.
///
/// # Other hazards
///
/// Stdin could be redirected from arbitrary input sources, and unless one
/// knows how the process' standard input is being used, one could consume
/// bytes that are expected to be consumed by other parts of the process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stdin.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stdin.3.html
#[inline]
pub unsafe fn stdin() -> BorrowedFd<'static> {
    BorrowedFd::borrow_raw(imp::io::STDIN_FILENO as RawFd)
}

/// `STDIN_FILENO`—Standard input, owned.
///
/// This is similar to [`stdin`], however it returns an `OwnedFd` which closes
/// standard input when it is dropped.
///
/// # Safety
///
/// This is unsafe for the same reasons as [`stdin`].
///
/// # Other hazards
///
/// This has the same hazards as [`stdin`].
///
/// And, when the `OwnedFd` is dropped, subsequent newly created file
/// descriptors may unknowingly reuse the stdin file descriptor number, which
/// may break common assumptions, so it should typically only be dropped at the
/// end of a program when no more file descriptors will be created.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stdin.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stdin.3.html
#[inline]
pub unsafe fn take_stdin() -> OwnedFd {
    OwnedFd::from(imp::fd::OwnedFd::from_raw_fd(
        imp::io::STDIN_FILENO as RawFd,
    ))
}

/// `STDOUT_FILENO`—Standard output, borrowed.
///
/// # Safety
///
/// This function must be called from code which knows how the process'
/// standard output is being used. Often, this will be the `main` function or
/// code that knows its relationship with the `main` function.
///
/// The stdout file descriptor can be closed, potentially on other threads, in
/// which case the file descriptor index value could be dynamically reused for
/// other purposes, potentially on different threads.
///
/// # Other hazards
///
/// Stdout could be redirected to arbitrary output sinks, and unless one
/// knows how the process' standard output is being used, one could
/// unexpectedly inject bytes into a stream being written by another part of
/// the process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stdout.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stdout.3.html
#[inline]
pub unsafe fn stdout() -> BorrowedFd<'static> {
    BorrowedFd::borrow_raw(imp::io::STDOUT_FILENO as RawFd)
}

/// `STDOUT_FILENO`—Standard output, owned.
///
/// This is similar to [`stdout`], however it returns an `OwnedFd` which closes
/// standard output when it is dropped.
///
/// # Safety
///
/// This is unsafe for the same reasons as [`stdout`].
///
/// # Other hazards
///
/// This has the same hazards as [`stdout`].
///
/// And, when the `OwnedFd` is dropped, subsequent newly created file
/// descriptors may unknowingly reuse the stdout file descriptor number, which
/// may break common assumptions, so it should typically only be dropped at the
/// end of a program when no more file descriptors will be created.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stdout.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stdout.3.html
#[inline]
pub unsafe fn take_stdout() -> OwnedFd {
    OwnedFd::from(imp::fd::OwnedFd::from_raw_fd(
        imp::io::STDOUT_FILENO as RawFd,
    ))
}

/// `STDERR_FILENO`—Standard error, borrowed.
///
/// # Safety
///
/// This function must be called from code which knows how the process'
/// standard error is being used. Often, this will be the `main` function or
/// code that knows its relationship with the `main` function.
///
/// The stderr file descriptor can be closed, potentially on other threads, in
/// which case the file descriptor index value could be dynamically reused for
/// other purposes, potentially on different threads.
///
/// # Other hazards
///
/// Stderr could be redirected to arbitrary output sinks, and unless one
/// knows how the process' standard error is being used, one could unexpectedly
/// inject bytes into a stream being written by another part of the process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stderr.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stderr.3.html
#[inline]
pub unsafe fn stderr() -> BorrowedFd<'static> {
    BorrowedFd::borrow_raw(imp::io::STDERR_FILENO as RawFd)
}

/// `STDERR_FILENO`—Standard error, owned.
///
/// This is similar to [`stdout`], however it returns an `OwnedFd` which closes
/// standard output when it is dropped.
///
/// # Safety
///
/// This is unsafe for the same reasons as [`stderr`].
///
/// # Other hazards
///
/// This has the same hazards as [`stderr`].
///
/// And, when the `OwnedFd` is dropped, subsequent newly created file
/// descriptors may unknowingly reuse the stderr file descriptor number, which
/// may break common assumptions, so it should typically only be dropped at the
/// end of a program when no more file descriptors will be created.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/stderr.html
/// [Linux]: https://man7.org/linux/man-pages/man3/stderr.3.html
#[inline]
pub unsafe fn take_stderr() -> OwnedFd {
    OwnedFd::from(imp::fd::OwnedFd::from_raw_fd(
        imp::io::STDERR_FILENO as RawFd,
    ))
}
