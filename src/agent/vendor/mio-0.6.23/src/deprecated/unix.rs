use {io, sys, Ready, Poll, PollOpt, Token};
use event::Evented;
use deprecated::TryAccept;
use io::MapNonBlock;
use std::io::{Read, Write};
use std::path::Path;
pub use std::net::Shutdown;
use std::process;

pub use sys::Io;

#[derive(Debug)]
pub struct UnixSocket {
    sys: sys::UnixSocket,
}

impl UnixSocket {
    /// Returns a new, unbound, non-blocking Unix domain socket
    pub fn stream() -> io::Result<UnixSocket> {
        sys::UnixSocket::stream()
            .map(From::from)
    }

    /// Connect the socket to the specified address
    pub fn connect<P: AsRef<Path> + ?Sized>(self, addr: &P) -> io::Result<(UnixStream, bool)> {
        let complete = match self.sys.connect(addr) {
            Ok(()) => true,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => false,
            Err(e) => return Err(e),
        };
        Ok((From::from(self.sys), complete))
    }

    /// Bind the socket to the specified address
    pub fn bind<P: AsRef<Path> + ?Sized>(&self, addr: &P) -> io::Result<()> {
        self.sys.bind(addr)
    }

    /// Listen for incoming requests
    pub fn listen(self, backlog: usize) -> io::Result<UnixListener> {
        self.sys.listen(backlog)?;
        Ok(From::from(self.sys))
    }

    pub fn try_clone(&self) -> io::Result<UnixSocket> {
        self.sys.try_clone()
            .map(From::from)
    }
}

impl Evented for UnixSocket {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.sys.deregister(poll)
    }
}

impl From<sys::UnixSocket> for UnixSocket {
    fn from(sys: sys::UnixSocket) -> UnixSocket {
        UnixSocket { sys }
    }
}

/*
 *
 * ===== UnixStream =====
 *
 */

#[derive(Debug)]
pub struct UnixStream {
    sys: sys::UnixSocket,
}

impl UnixStream {
    pub fn connect<P: AsRef<Path> + ?Sized>(path: &P) -> io::Result<UnixStream> {
        UnixSocket::stream()
            .and_then(|sock| sock.connect(path))
            .map(|(sock, _)| sock)
    }

    pub fn try_clone(&self) -> io::Result<UnixStream> {
        self.sys.try_clone()
            .map(From::from)
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<usize> {
        self.sys.shutdown(how).map(|_| 0)
    }

    pub fn read_recv_fd(&mut self, buf: &mut [u8]) -> io::Result<(usize, Option<RawFd>)> {
        self.sys.read_recv_fd(buf)
    }

    pub fn try_read_recv_fd(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, Option<RawFd>)>> {
        self.read_recv_fd(buf).map_non_block()
    }

    pub fn write_send_fd(&mut self, buf: &[u8], fd: RawFd) -> io::Result<usize> {
        self.sys.write_send_fd(buf, fd)
    }

    pub fn try_write_send_fd(&mut self, buf: &[u8], fd: RawFd) -> io::Result<Option<usize>> {
        self.write_send_fd(buf, fd).map_non_block()
    }
}

impl Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.sys.read(buf)
    }
}

impl Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sys.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sys.flush()
    }
}

impl Evented for UnixStream {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.sys.deregister(poll)
    }
}

impl From<sys::UnixSocket> for UnixStream {
    fn from(sys: sys::UnixSocket) -> UnixStream {
        UnixStream { sys }
    }
}

/*
 *
 * ===== UnixListener =====
 *
 */

#[derive(Debug)]
pub struct UnixListener {
    sys: sys::UnixSocket,
}

impl UnixListener {
    pub fn bind<P: AsRef<Path> + ?Sized>(addr: &P) -> io::Result<UnixListener> {
        UnixSocket::stream().and_then(|sock| {
            sock.bind(addr)?;
            sock.listen(256)
        })
    }

    pub fn accept(&self) -> io::Result<UnixStream> {
        self.sys.accept().map(From::from)
    }

    pub fn try_clone(&self) -> io::Result<UnixListener> {
        self.sys.try_clone().map(From::from)
    }
}

impl Evented for UnixListener {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.sys.deregister(poll)
    }
}

impl TryAccept for UnixListener {
    type Output = UnixStream;

    fn accept(&self) -> io::Result<Option<UnixStream>> {
        UnixListener::accept(self).map_non_block()
    }
}

impl From<sys::UnixSocket> for UnixListener {
    fn from(sys: sys::UnixSocket) -> UnixListener {
        UnixListener { sys }
    }
}

/*
 *
 * ===== Pipe =====
 *
 */

pub fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    let (rd, wr) = sys::pipe()?;
    Ok((From::from(rd), From::from(wr)))
}

#[derive(Debug)]
pub struct PipeReader {
    io: Io,
}

impl PipeReader {
    pub fn from_stdout(stdout: process::ChildStdout) -> io::Result<Self> {
        if let Err(e) = sys::set_nonblock(stdout.as_raw_fd()) {
            return Err(e);
        }
        Ok(PipeReader::from(unsafe { Io::from_raw_fd(stdout.into_raw_fd()) }))
    }
    pub fn from_stderr(stderr: process::ChildStderr) -> io::Result<Self> {
        if let Err(e) = sys::set_nonblock(stderr.as_raw_fd()) {
            return Err(e);
        }
        Ok(PipeReader::from(unsafe { Io::from_raw_fd(stderr.into_raw_fd()) }))
    }
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read(buf)
    }
}

impl<'a> Read for &'a PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.io).read(buf)
    }
}

impl Evented for PipeReader {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.io.deregister(poll)
    }
}

impl From<Io> for PipeReader {
    fn from(io: Io) -> PipeReader {
        PipeReader { io }
    }
}

#[derive(Debug)]
pub struct PipeWriter {
    io: Io,
}

impl PipeWriter {
    pub fn from_stdin(stdin: process::ChildStdin) -> io::Result<Self> {
        if let Err(e) = sys::set_nonblock(stdin.as_raw_fd()) {
            return Err(e);
        }
        Ok(PipeWriter::from(unsafe { Io::from_raw_fd(stdin.into_raw_fd()) }))
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl<'a> Write for &'a PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.io).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.io).flush()
    }
}

impl Evented for PipeWriter {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.io.deregister(poll)
    }
}

impl From<Io> for PipeWriter {
    fn from(io: Io) -> PipeWriter {
        PipeWriter { io }
    }
}

/*
 *
 * ===== Conversions =====
 *
 */

use std::os::unix::io::{RawFd, IntoRawFd, AsRawFd, FromRawFd};

impl IntoRawFd for UnixSocket {
    fn into_raw_fd(self) -> RawFd {
        self.sys.into_raw_fd()
    }
}

impl AsRawFd for UnixSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.sys.as_raw_fd()
    }
}

impl FromRawFd for UnixSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixSocket {
        UnixSocket { sys: FromRawFd::from_raw_fd(fd) }
    }
}

impl IntoRawFd for UnixStream {
    fn into_raw_fd(self) -> RawFd {
        self.sys.into_raw_fd()
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.sys.as_raw_fd()
    }
}

impl FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixStream {
        UnixStream { sys: FromRawFd::from_raw_fd(fd) }
    }
}

impl IntoRawFd for UnixListener {
    fn into_raw_fd(self) -> RawFd {
        self.sys.into_raw_fd()
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.sys.as_raw_fd()
    }
}

impl FromRawFd for UnixListener {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixListener {
        UnixListener { sys: FromRawFd::from_raw_fd(fd) }
    }
}

impl IntoRawFd for PipeReader {
    fn into_raw_fd(self) -> RawFd {
        self.io.into_raw_fd()
    }
}

impl AsRawFd for PipeReader {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl FromRawFd for PipeReader {
    unsafe fn from_raw_fd(fd: RawFd) -> PipeReader {
        PipeReader { io: FromRawFd::from_raw_fd(fd) }
    }
}

impl IntoRawFd for PipeWriter {
    fn into_raw_fd(self) -> RawFd {
        self.io.into_raw_fd()
    }
}

impl AsRawFd for PipeWriter {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl FromRawFd for PipeWriter {
    unsafe fn from_raw_fd(fd: RawFd) -> PipeWriter {
        PipeWriter { io: FromRawFd::from_raw_fd(fd) }
    }
}
