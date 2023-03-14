#![cfg_attr(not(rustc_attrs), allow(unused_imports))]
#![cfg_attr(target_os = "wasi", feature(wasi_ext))]
#![cfg_attr(io_lifetimes_use_std, feature(io_safety))]

use std::mem::size_of;

#[cfg(any(unix, target_os = "wasi"))]
use io_lifetimes::{BorrowedFd, OwnedFd};
#[cfg(windows)]
use io_lifetimes::{BorrowedHandle, BorrowedSocket, OwnedHandle, OwnedSocket};

#[cfg(unix)]
use std::os::unix::io::RawFd;
#[cfg(target_os = "wasi")]
use std::os::wasi::io::RawFd;
#[cfg(windows)]
use std::os::windows::io::{RawHandle, RawSocket};

#[cfg(all(rustc_attrs, any(unix, target_os = "wasi")))]
#[test]
fn test_niche_optimizations() {
    assert_eq!(size_of::<Option<OwnedFd>>(), size_of::<RawFd>());
    assert_eq!(size_of::<Option<BorrowedFd<'static>>>(), size_of::<RawFd>());
}

#[cfg(all(rustc_attrs, windows))]
#[test]
fn test_niche_optimizations_socket() {
    assert_eq!(size_of::<Option<OwnedSocket>>(), size_of::<RawSocket>());
    assert_eq!(
        size_of::<Option<BorrowedSocket<'static>>>(),
        size_of::<RawSocket>(),
    );
}
