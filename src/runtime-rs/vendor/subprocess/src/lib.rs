//! Execution of and interaction with external processes and pipelines.
//!
//! The entry points to the crate are the [`Popen`] struct and the [`Exec`]
//! builder.  `Popen` is the interface to a running child process, inspired by
//! Python's [`subprocess.Popen`].  `Exec` provides a builder-pattern API with
//! convenient methods for streaming and capturing of output, as well as
//! combining `Popen` instances into pipelines.
//!
//! Compared to `std::process`, the module follows the following
//! additional features:
//!
//! * The *communicate* [family of methods] for deadlock-free capturing of
//!   subprocess output/error, while simultaneously feeding data to its
//!   standard input.  Capturing supports optional timeout and read size
//!   limit.
//!
//! * Connecting multiple commands into OS-level [pipelines].
//!
//! * Flexible [redirection options], such as connecting standard
//!   streams to arbitary [open files], or [merging] output streams
//!   like shell's `2>&1` and `1>&2` operators.
//!
//! * Non-blocking and timeout methods to wait on the process:
//!   [`poll`], [`wait`], and [`wait_timeout`].
//!
//! # Examples
//!
//! Communicate with a process and optionally terminate it:
//!
//! ```
//! # use subprocess::*;
//! # fn dummy() -> Result<()> {
//! let mut p = Popen::create(&["ps", "x"], PopenConfig {
//!     stdout: Redirection::Pipe, ..Default::default()
//! })?;
//!
//! // Obtain the output from the standard streams.
//! let (out, err) = p.communicate(None)?;
//!
//! if let Some(exit_status) = p.poll() {
//!     // the process has finished
//! } else {
//!     // it is still running, terminate it
//!     p.terminate()?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Use the [`Exec`] builder to execute a pipeline of commands and
//! capture the output:
//!
//! ```no_run
//! # use subprocess::*;
//! # fn dummy() -> Result<()> {
//! let dir_checksum = {
//!     Exec::shell("find . -type f") | Exec::cmd("sort") | Exec::cmd("sha1sum")
//! }.capture()?.stdout_str();
//! # Ok(())
//! # }
//! ```
//!
//! [`Popen`]: struct.Popen.html
//! [`Exec`]: struct.Exec.html
//! [family of methods]: struct.Popen.html#method.communicate_start
//! [redirection options]: enum.Redirection.html
//! [open files]: enum.Redirection.html#variant.File
//! [merging]: enum.Redirection.html#variant.Merge
//! [`poll`]: struct.Popen.html#method.poll
//! [`wait`]: struct.Popen.html#method.wait
//! [`wait_timeout`]: struct.Popen.html#method.wait_timeout
//! [`subprocess.Popen`]: https://docs.python.org/3/library/subprocess.html#subprocess.Popen
//! [pipelines]: struct.Pipeline.html

#![warn(missing_debug_implementations, rust_2018_idioms, missing_docs)]
#![allow(clippy::type_complexity, clippy::single_match)]

mod builder;
mod communicate;
mod popen;

#[cfg(unix)]
mod posix;

#[cfg(windows)]
mod win32;

mod os_common;

pub use self::builder::{CaptureData, Exec, NullFile, Pipeline};
pub use self::communicate::{CommunicateError, Communicator};
pub use self::os_common::ExitStatus;
pub use self::popen::{make_pipe, Popen, PopenConfig, PopenError, Redirection, Result};

/// Subprocess extensions for Unix platforms.
pub mod unix {
    pub use super::popen::os_ext::*;
}

#[cfg(test)]
mod tests {
    mod builder;
    mod common;
    #[cfg(unix)]
    mod posix;
    #[cfg(windows)]
    mod win32;
}
