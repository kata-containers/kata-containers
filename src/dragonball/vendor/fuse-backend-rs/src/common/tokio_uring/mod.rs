//! Tokio-uring provides a safe [io-uring] interface for the Tokio runtime. The
//! library requires Linux kernel 5.10 or later.
//!
//! [io-uring]: https://kernel.dk/io_uring.pdf
//!
//! # Getting started
//!
//! Using `tokio-uring` requires starting a [`tokio-uring`] runtime. This
//! runtime internally manages the main Tokio runtime and a `io-uring` driver.
//!
//! ```no_run
//! use tokio_uring::fs::File;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     tokio_uring::start(async {
//!         // Open a file
//!         let file = File::open("hello.txt").await?;
//!
//!         let buf = vec![0; 4096];
//!         // Read some data, the buffer is passed by ownership and
//!         // submitted to the kernel. When the operation completes,
//!         // we get the buffer back.
//!         let (res, buf) = file.read_at(buf, 0).await;
//!         let n = res?;
//!
//!         // Display the contents
//!         println!("{:?}", &buf[..n]);
//!
//!         Ok(())
//!     })
//! }
//! ```
//!
//! Under the hood, `tokio_uring::start` starts a [`current-thread`] Runtime.
//! For concurrency, spawn multiple threads, each with a `tokio-uring` runtime.
//! The `tokio-uring` resource types are optimized for single-threaded usage and
//! most are `!Sync`.
//!
//! # Submit-based operations
//!
//! Unlike Tokio proper, `io-uring` is based on submission based operations.
//! Ownership of resources are passed to the kernel, which then performs the
//! operation. When the operation completes, ownership is passed back to the
//! caller. Because of this difference, the `tokio-uring` APIs diverge.
//!
//! For example, in the above example, reading from a `File` requires passing
//! ownership of the buffer.
//!
//! # Closing resources
//!
//! With `io-uring`, closing a resource (e.g. a file) is an asynchronous
//! operation. Because Rust does not support asynchronous drop yet, resource
//! types provide an explicit `close()` function. If the `close()` function is
//! not called, the resource will still be closed on drop, but the operation
//! will happen in the background. There is no guarantee as to **when** the
//! implicit close-on-drop operation happens, so it is recommended to explicitly
//! call `close()`.

#![warn(missing_docs)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::needless_doctest_main)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_return)]

macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

#[macro_use]
pub(crate) mod future;
pub(crate) mod driver;
mod runtime;

pub mod buf;
pub mod fs;
pub mod net;

pub use runtime::{spawn, Runtime};

use std::future::Future;

/// Start an `io_uring` enabled Tokio runtime.
///
/// All `tokio-uring` resource types must be used from within the context of a
/// runtime. The `start` method initializes the runtime and runs it for the
/// duration of `future`.
///
/// The `tokio-uring` runtime is compatible with all Tokio, so it is possible to
/// run Tokio based libraries (e.g. hyper) from within the tokio-uring runtime.
/// A `tokio-uring` runtime consists of a Tokio `current_thread` runtime and an
/// `io-uring` driver. All tasks spawned on the `tokio-uring` runtime are
/// executed on the current thread. To add concurrency, spawn multiple threads,
/// each with a `tokio-uring` runtime.
///
/// # Examples
///
/// Basic usage
///
/// ```no_run
/// use tokio_uring::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         // Open a file
///         let file = File::open("hello.txt").await?;
///
///         let buf = vec![0; 4096];
///         // Read some data, the buffer is passed by ownership and
///         // submitted to the kernel. When the operation completes,
///         // we get the buffer back.
///         let (res, buf) = file.read_at(buf, 0).await;
///         let n = res?;
///
///         // Display the contents
///         println!("{:?}", &buf[..n]);
///
///         Ok(())
///     })
/// }
/// ```
///
/// Using Tokio types from the `tokio-uring` runtime
///
///
/// ```no_run
/// use tokio::net::TcpListener;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         let listener = TcpListener::bind("127.0.0.1:8080").await?;
///
///         loop {
///             let (socket, _) = listener.accept().await?;
///             // process socket
///         }
///     })
/// }
/// ```
pub fn start<F: Future>(future: F) -> F::Output {
    let mut rt = runtime::Runtime::new().unwrap();
    rt.block_on(future)
}

/// A specialized `Result` type for `io-uring` operations with buffers.
///
/// This type is used as a return value for asynchronous `io-uring` methods that
/// require passing ownership of a buffer to the runtime. When the operation
/// completes, the buffer is returned whether or not the operation completed
/// successfully.
///
/// # Examples
///
/// ```no_run
/// use tokio_uring::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         // Open a file
///         let file = File::open("hello.txt").await?;
///
///         let buf = vec![0; 4096];
///         // Read some data, the buffer is passed by ownership and
///         // submitted to the kernel. When the operation completes,
///         // we get the buffer back.
///         let (res, buf) = file.read_at(buf, 0).await;
///         let n = res?;
///
///         // Display the contents
///         println!("{:?}", &buf[..n]);
///
///         Ok(())
///     })
/// }
/// ```
pub type BufResult<T, B> = (std::io::Result<T>, B);
