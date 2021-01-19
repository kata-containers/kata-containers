// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Async support for pipe or something has file descriptor

use std::{
    fmt, io, mem,
    os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll as MioPoll, PollOpt, Ready, Token};
use nix::unistd;
use tokio::io::{AsyncRead, AsyncWrite, PollEvented};

unsafe fn set_nonblocking(fd: RawFd) {
    libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
}

struct StreamFd(RawFd);

impl Evented for StreamFd {
    fn register(
        &self,
        poll: &MioPoll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &MioPoll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &MioPoll) -> io::Result<()> {
        EventedFd(&self.0).deregister(poll)
    }
}

impl io::Read for StreamFd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match unistd::read(self.0, buf) {
            Ok(l) => Ok(l),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }
}

impl io::Write for StreamFd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match unistd::write(self.0, buf) {
            Ok(l) => Ok(l),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl StreamFd {
    fn close(&mut self) -> io::Result<()> {
        match unistd::close(self.0) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }
}

impl Drop for StreamFd {
    fn drop(&mut self) {
        self.close().ok();
    }
}

/// Pipe read
pub struct PipeStream(PollEvented<StreamFd>);

impl PipeStream {
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.0.get_mut().close()
    }

    pub fn from_fd(fd: RawFd) -> Self {
        unsafe { Self::from_raw_fd(fd) }
    }
}

impl AsyncRead for PipeStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsRawFd for PipeStream {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().0
    }
}

impl IntoRawFd for PipeStream {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.0.get_ref().0;
        mem::forget(self);
        fd
    }
}

impl FromRawFd for PipeStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        set_nonblocking(fd);
        PipeStream(PollEvented::new(StreamFd(fd)).unwrap())
    }
}

impl fmt::Debug for PipeStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PipeStream({})", self.as_raw_fd())
    }
}

impl AsyncWrite for PipeStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
