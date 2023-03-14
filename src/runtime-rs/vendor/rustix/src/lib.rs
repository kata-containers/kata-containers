//! `rustix` provides efficient memory-safe and [I/O-safe] wrappers to
//! POSIX-like, Unix-like, Linux, and Winsock2 syscall-like APIs, with
//! configurable backends.
//!
//! With rustix, you can write code like this:
//!
//! ```rust
//! # fn read(sock: std::net::TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
//! # use rustix::net::RecvFlags;
//! let nread: usize = rustix::net::recv(&sock, buf, RecvFlags::PEEK)?;
//! # let _ = nread;
//! # Ok(())
//! # }
//! ```
//!
//! instead of like this:
//!
//! ```rust
//! # fn read(sock: std::net::TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
//! # use std::convert::TryInto;
//! # use rustix::fd::AsRawFd;
//! # #[cfg(windows)]
//! # use winapi::um::winsock2 as libc;
//! # const MSG_PEEK: i32 = libc::MSG_PEEK;
//! let nread: usize = unsafe {
//!     match libc::recv(
//!         sock.as_raw_fd() as _,
//!         buf.as_mut_ptr().cast(),
//!         buf.len().try_into().unwrap_or(i32::MAX as _),
//!         MSG_PEEK,
//!     ) {
//!         -1 => return Err(std::io::Error::last_os_error()),
//!         nread => nread as usize,
//!     }
//! };
//! # let _ = nread;
//! # Ok(())
//! # }
//! ```
//!
//! rustix's APIs perform the following tasks:
//!  - Error values are translated to [`Result`]s.
//!  - Buffers are passed as Rust slices.
//!  - Out-parameters are presented as return values.
//!  - Path arguments use [`Arg`], so they accept any string type.
//!  - File descriptors are passed and returned via [`AsFd`] and [`OwnedFd`]
//!    instead of bare integers, ensuring I/O safety.
//!  - Constants use `enum`s and [`bitflags`] types.
//!  - Multiplexed functions (eg. `fcntl`, `ioctl`, etc.) are de-multiplexed.
//!  - Variadic functions (eg. `openat`, etc.) are presented as non-variadic.
//!  - Functions and types which need `l` prefixes or `64` suffixes to enable
//!    large-file support are used automatically, and file sizes and offsets
//!    are presented as `i64` and `u64`.
//!  - Behaviors that depend on the sizes of C types like `long` are hidden.
//!  - In some places, more human-friendly and less historical-accident names
//!    are used (and documentation aliases are used so that the original names
//!    can still be searched for).
//!
//! Things they don't do include:
//!  - Detecting whether functions are supported at runtime.
//!  - Hiding significant differences between platforms.
//!  - Restricting ambient authorities.
//!  - Imposing sandboxing features such as filesystem path or network address
//!    sandboxing.
//!
//! See [`cap-std`], [`system-interface`], and [`io-streams`] for libraries
//! which do hide significant differences between platforms, and [`cap-std`]
//! which does perform sandboxing and restricts ambient authorities.
//!
//! [`cap-std`]: https://crates.io/crates/cap-std
//! [`system-interface`]: https://crates.io/crates/system-interface
//! [`io-streams`]: https://crates.io/crates/io-streams
//! [`getrandom`]: https://crates.io/crates/getrandom
//! [`bitflags`]: https://crates.io/crates/bitflags
//! [`AsFd`]: https://doc.rust-lang.org/stable/std/os/unix/io/trait.AsFd.html
//! [`OwnedFd`]: https://docs.rs/io-lifetimes/latest/io_lifetimes/struct.OwnedFd.html
//! [io-lifetimes crate]: https://crates.io/crates/io-lifetimes
//! [I/O-safe]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
//! [`Result`]: https://docs.rs/rustix/latest/rustix/io/type.Result.html
//! [`Arg`]: https://docs.rs/rustix/latest/rustix/path/trait.Arg.html

#![deny(missing_docs)]
#![allow(stable_features)]
#![cfg_attr(linux_raw, deny(unsafe_code))]
#![cfg_attr(rustc_attrs, feature(rustc_attrs))]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![cfg_attr(all(target_os = "wasi", feature = "std"), feature(wasi_ext))]
#![cfg_attr(
    all(linux_raw, naked_functions, target_arch = "x86"),
    feature(naked_functions)
)]
#![cfg_attr(io_lifetimes_use_std, feature(io_safety))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(all(not(feature = "std"), specialization), allow(incomplete_features))]
#![cfg_attr(all(not(feature = "std"), specialization), feature(specialization))]
#![cfg_attr(all(not(feature = "std"), slice_internals), feature(slice_internals))]
#![cfg_attr(
    all(not(feature = "std"), toowned_clone_into),
    feature(toowned_clone_into)
)]
#![cfg_attr(
    all(not(feature = "std"), vec_into_raw_parts),
    feature(vec_into_raw_parts)
)]
#![cfg_attr(feature = "rustc-dep-of-std", feature(core_intrinsics))]
#![cfg_attr(feature = "rustc-dep-of-std", feature(ip))]
#![cfg_attr(
    all(not(feature = "rustc-dep-of-std"), core_intrinsics),
    feature(core_intrinsics)
)]
#![cfg_attr(asm_experimental_arch, feature(asm_experimental_arch))]

#[cfg(not(feature = "rustc-dep-of-std"))]
extern crate alloc;

/// Export `*Fd` types and traits that used in rustix's public API.
///
/// Users can use this to avoid needing to import anything else to use the same
/// versions of these types and traits.
///
/// Note that rustix APIs that use `OwnedFd` use [`rustix::io::OwnedFd`]
/// instead, which allows rustix to implement `close` for them.
///
/// [`rustix::io::OwnedFd`]: crate::io::OwnedFd
pub mod fd {
    use super::imp;
    pub use imp::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
    #[cfg(windows)]
    pub use imp::fd::{AsSocket, FromSocket, IntoSocket};
    #[cfg(feature = "std")]
    pub use imp::fd::{FromFd, IntoFd};
}

#[cfg(not(windows))]
#[macro_use]
pub(crate) mod zstr;
#[macro_use]
pub(crate) mod const_assert;

#[cfg_attr(libc, path = "imp/libc/mod.rs")]
#[cfg_attr(linux_raw, path = "imp/linux_raw/mod.rs")]
#[cfg_attr(wasi, path = "imp/wasi/mod.rs")]
mod imp;

#[cfg(not(windows))]
pub mod ffi;
#[cfg(not(windows))]
pub mod fs;
pub mod io;
#[cfg(any(target_os = "android", target_os = "linux"))]
#[cfg(feature = "io_uring")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "io_uring")))]
pub mod io_uring;
#[cfg(not(any(target_os = "redox", target_os = "wasi")))] // WASI doesn't support `net` yet.
pub mod net;
#[cfg(not(windows))]
pub mod path;
#[cfg(not(windows))]
pub mod process;
#[cfg(not(windows))]
pub mod rand;
#[cfg(not(windows))]
pub mod thread;
#[cfg(not(windows))]
pub mod time;

#[cfg(not(windows))]
#[doc(hidden)]
pub mod runtime;

/// Convert a `&T` into a `*const T` without using an `as`.
#[inline]
#[allow(dead_code)]
const fn as_ptr<T>(t: &T) -> *const T {
    t
}

/// Convert a `&mut T` into a `*mut T` without using an `as`.
#[inline]
#[allow(dead_code)]
fn as_mut_ptr<T>(t: &mut T) -> *mut T {
    t
}
