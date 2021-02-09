//! Module with the self-pipe pattern.
//!
//! One of the common patterns around signals is to have a pipe with both ends in the same program.
//! Whenever there's a signal, the signal handler writes one byte of garbage data to the write end,
//! unless the pipe's already full. The application then can handle the read end.
//!
//! This has two advantages. First, the real signal action moves outside of the signal handler
//! where there are a lot less restrictions. Second, it fits nicely in all kinds of asynchronous
//! loops and has less chance of race conditions.
//!
//! This module offers premade functions for the write end (and doesn't insist that it must be a
//! pipe ‒ anything that can be written to is fine ‒ sockets too, therefore `UnixStream::pair` is a
//! good candidate).
//!
//! If you want to integrate with some asynchronous library, plugging streams from `mio-uds` or
//! `tokio-uds` libraries should work.
//!
//! If it looks too low-level for your needs, the [`iterator`](../iterator/) module contains some
//! higher-lever interface that also uses a self-pipe pattern under the hood.
//!
//! # Correct order of handling
//!
//! A care needs to be taken to avoid race conditions, especially when handling the same signal in
//! a loop. Specifically, another signal might come when the action for the previous signal is
//! being taken. The correct order is first to clear the content of the pipe (read some/all data
//! from it) and then take the action. This way a spurious wakeup can happen (the pipe could wake
//! up even when no signal came after the signal was taken, because ‒ it arrived between cleaning
//! the pipe and taking the action). Note that some OS primitives (eg. `select`) suffer from
//! spurious wakeups themselves (they can claim a FD is readable when it is not true) and blocking
//! `read` might return prematurely (with eg. `EINTR`).
//!
//! The reverse order of first taking the action and then clearing the pipe might lose signals,
//! which is usually worse.
//!
//! This is not a problem with blocking on reading from the pipe (because both the blocking and
//! cleaning is the same action), but in case of asynchronous handling it matters.
//!
//! If you want to combine setting some flags with a self-pipe pattern, the flag needs to be set
//! first, then the pipe written. On the read end, first the pipe needs to be cleaned, then the
//! flag and then the action taken. This is what the [`Signals`](../iterator/struct.Signals.html)
//! structure does internally.
//!
//! # Write collating
//!
//! While unlikely if handled correctly, it is possible the write end is full when a signal comes.
//! In such case the signal handler simply does nothing. If the write end is full, the read end is
//! readable and therefore will wake up. On the other hand, blocking in the signal handler would
//! definitely be a bad idea.
//!
//! However, this also means the number of bytes read from the end might be lower than the number
//! of signals that arrived. This should not generally be a problem, since the OS already collates
//! signals of the same kind together.
//!
//! # Examples
//!
//! This example waits for at last one `SIGUSR1` signal to come before continuing (and
//! terminating). It sends the signal to itself, so it correctly terminates.
//!
//! ```rust
//! extern crate libc;
//! extern crate signal_hook;
//!
//! use std::io::{Error, Read};
//! use std::os::unix::net::UnixStream;
//!
//! fn main() -> Result<(), Error> {
//!     let (mut read, write) = UnixStream::pair()?;
//!     signal_hook::pipe::register(signal_hook::SIGUSR1, write)?;
//!     // This will write into the pipe write end through the signal handler
//!     unsafe { libc::raise(signal_hook::SIGUSR1) };
//!     let mut buff = [0];
//!     read.read_exact(&mut buff)?;
//!     println!("Happily terminating");
//!     Ok(())
//! }

use std::io::Error;
use std::os::unix::io::{AsRawFd, RawFd};

use libc::{self, c_int};

use crate::SigId;

struct OwnedFd(RawFd);

impl OwnedFd {
    /// Sets close on exec and nonblock on the inner file descriptor.
    fn set_flags(&self) -> Result<(), Error> {
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFL, 0);
            if flags == -1 {
                return Err(Error::last_os_error());
            }
            let flags = flags | libc::O_NONBLOCK | libc::O_CLOEXEC;
            if libc::fcntl(self.as_raw_fd(), libc::F_SETFL, flags) == -1 {
                return Err(Error::last_os_error());
            }
        }
        Ok(())
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

pub(crate) fn wake(pipe: RawFd) {
    unsafe {
        // This writes some data into the pipe.
        //
        // There are two tricks:
        // * First, the crazy cast. The first part turns reference into pointer. The second part
        //   turns pointer to u8 into a pointer to void, which is what write requires.
        // * Second, we ignore errors, on purpose. We don't have any means to handling them. The
        //   two conceivable errors are EBADFD, if someone passes a non-existent file descriptor or
        //   if it is closed. The second is EAGAIN, in which case the pipe is full ‒ there were
        //   many signals, but the reader didn't have time to read the data yet. It'll still get
        //   woken up, so not fitting another letter in it is fine.
        libc::write(pipe, b"X" as *const _ as *const _, 1);
    }
}

/// Registers a write to a self-pipe whenever there's the signal.
///
/// In this case, the pipe is taken as the `RawFd`. It is still the caller's responsibility to
/// close it.
///
/// Note that passing the wrong file descriptor won't cause UB, but can still lead to severe bugs ‒
/// like data corruptions in files.
pub fn register_raw(signal: c_int, pipe: RawFd) -> Result<SigId, Error> {
    // A trick here:
    // We want to set the FD non-blocking. But it belongs to the caller. Therefore, we make our own
    // copy with `dup` to play on instead.
    let duped = unsafe { libc::dup(pipe) };
    if duped == -1 {
        return Err(Error::last_os_error());
    }
    let duped = OwnedFd(duped);
    duped.set_flags()?;
    let action = move || wake(duped.as_raw_fd());
    unsafe { crate::register(signal, action) }
}

/// Registers a write to a self-pipe whenever there's the signal.
///
/// The ownership of pipe is taken and will be closed whenever the created action is unregistered.
///
/// Note that if you want to register the same pipe for multiple signals, there's `try_clone`
/// method on many unix socket primitives.
pub fn register<P>(signal: c_int, pipe: P) -> Result<SigId, Error>
where
    P: AsRawFd + Send + Sync + 'static,
{
    let id = register_raw(signal, pipe.as_raw_fd())?;
    // Close the original
    drop(pipe);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::unix::net::{UnixDatagram, UnixStream};

    use super::*;

    // Note: multiple tests share the SIGUSR1 signal. This is fine, we only need to know the signal
    // arrives. It's OK to arrive multiple times, from multiple tests.
    fn wakeup() {
        unsafe { assert_eq!(0, libc::raise(libc::SIGUSR1)) }
    }

    #[test]
    fn register_with_socket() -> Result<(), Error> {
        let (mut read, write) = UnixStream::pair()?;
        register(libc::SIGUSR1, write)?;
        read.set_nonblocking(true)?;
        wakeup();
        let mut buff = [0; 1];
        read.read_exact(&mut buff)?;
        assert_eq!(b"X", &buff);
        Ok(())
    }

    #[test]
    fn register_dgram_socket() -> Result<(), Error> {
        let (read, write) = UnixDatagram::pair()?;
        register(libc::SIGUSR1, write)?;
        read.set_nonblocking(true)?;
        wakeup();
        let mut buff = [0; 1];
        read.recv(&mut buff)?;
        assert_eq!(b"X", &buff);
        Ok(())
    }

    #[test]
    fn register_with_pipe() -> Result<(), Error> {
        let mut fds = [0; 2];
        unsafe { assert_eq!(0, libc::pipe(fds.as_mut_ptr())) };
        let read = OwnedFd(fds[0]);
        let write = OwnedFd(fds[1]);
        register(libc::SIGUSR1, write)?;
        read.set_flags()?;
        wakeup();
        let mut buff = [0; 1];
        unsafe {
            assert_eq!(
                1,
                libc::read(read.as_raw_fd(), buff.as_mut_ptr() as *mut _, 1)
            )
        }
        assert_eq!(b"X", &buff);
        Ok(())
    }
}
