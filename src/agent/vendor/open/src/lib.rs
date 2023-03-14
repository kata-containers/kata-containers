//! Use this library to open a path or URL using the program configured on the system in a non-blocking fashion.
//!
//! # Usage
//!
//! Open the given URL in the default web browser, without blocking.
//!
//! ```no_run
//! open::that("http://rust-lang.org").unwrap();
//! ```
//!
//! Alternatively, specify the program to be used to open the path or URL.
//!
//! ```no_run
//! open::with("http://rust-lang.org", "firefox").unwrap();
//! ```
//!
//! # Notes
//!
//! ## Nonblocking operation
//!
//! The functions provided are nonblocking as it will return even though the
//! launched child process is still running. Note that depending on the operating
//! system, spawning launch helpers, which this library does under the hood,
//! might still take 100's of milliseconds.
//!
//! ## Error handling
//!
//! As an operating system program is used, the open operation can fail.
//! Therefore, you are advised to check the result and behave
//! accordingly, e.g. by letting the user know that the open operation failed.
//!
//! ```no_run
//! let path = "http://rust-lang.org";
//!
//! match open::that(path) {
//!     Ok(()) => println!("Opened '{}' successfully.", path),
//!     Err(err) => eprintln!("An error occurred when opening '{}': {}", path, err),
//! }
//! ```

#[cfg(target_os = "windows")]
use windows as os;

#[cfg(target_os = "macos")]
use macos as os;

#[cfg(target_os = "ios")]
use ios as os;

#[cfg(target_os = "haiku")]
use haiku as os;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "illumos",
    target_os = "solaris"
))]
use unix as os;

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "illumos",
    target_os = "solaris",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    target_os = "haiku"
)))]
compile_error!("open is not supported on this platform");

use std::{
    ffi::OsStr,
    io,
    process::{Command, Stdio},
    thread,
};

/// Open path with the default application without blocking.
///
/// # Examples
///
/// ```no_run
/// let path = "http://rust-lang.org";
///
/// match open::that(path) {
///     Ok(()) => println!("Opened '{}' successfully.", path),
///     Err(err) => panic!("An error occurred when opening '{}': {}", path, err),
/// }
/// ```
///
/// # Errors
///
/// A [`std::io::Error`] is returned on failure. Because different operating systems
/// handle errors differently it is recommend to not match on a certain error.
pub fn that<T: AsRef<OsStr>>(path: T) -> io::Result<()> {
    os::that(path)
}

/// Open path with the given application.
///
/// This function may block if the application doesn't detach itself.
/// In that case, consider using [`with_in_background()`].
///
/// # Examples
///
/// ```no_run
/// let path = "http://rust-lang.org";
/// let app = "firefox";
///
/// match open::with(path, app) {
///     Ok(()) => println!("Opened '{}' successfully.", path),
///     Err(err) => panic!("An error occurred when opening '{}': {}", path, err),
/// }
/// ```
///
/// # Errors
///
/// A [`std::io::Error`] is returned on failure. Because different operating systems
/// handle errors differently it is recommend to not match on a certain error.
pub fn with<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> io::Result<()> {
    os::with(path, app)
}

/// Open path with the default application in a new thread.
///
/// See documentation of [`that()`] for more details.
#[deprecated = "Use `that()` as it is non-blocking while making error handling easy."]
pub fn that_in_background<T: AsRef<OsStr>>(path: T) -> thread::JoinHandle<io::Result<()>> {
    let path = path.as_ref().to_os_string();
    thread::spawn(|| that(path))
}

/// Open path with the given application in a new thread, which is useful if
/// the program ends up to be blocking. Otherwise, prefer [`with()`] for
/// straightforward error handling.
///
/// See documentation of [`with()`] for more details.
pub fn with_in_background<T: AsRef<OsStr>>(
    path: T,
    app: impl Into<String>,
) -> thread::JoinHandle<io::Result<()>> {
    let path = path.as_ref().to_os_string();
    let app = app.into();
    thread::spawn(|| with(path, app))
}

trait IntoResult<T> {
    fn into_result(self) -> T;
}

impl IntoResult<io::Result<()>> for io::Result<std::process::ExitStatus> {
    fn into_result(self) -> io::Result<()> {
        match self {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Launcher failed with {:?}", status),
            )),
            Err(err) => Err(err),
        }
    }
}

#[cfg(windows)]
impl IntoResult<io::Result<()>> for std::os::raw::c_int {
    fn into_result(self) -> io::Result<()> {
        match self {
            i if i > 32 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

trait CommandExt {
    fn status_without_output(&mut self) -> io::Result<std::process::ExitStatus>;
}

impl CommandExt for Command {
    fn status_without_output(&mut self) -> io::Result<std::process::ExitStatus> {
        let mut process = self
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        process.wait()
    }
}

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_os = "haiku")]
mod haiku;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "illumos",
    target_os = "solaris"
))]
mod unix;
