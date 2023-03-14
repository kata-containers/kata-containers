#[cfg(feature = "async-io")]
use async_io::Async;
use futures_core::ready;
use std::{
    io,
    task::{Context, Poll},
};
#[cfg(feature = "async-io")]
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[cfg(all(windows, feature = "async-io"))]
use uds_windows::UnixStream;

#[cfg(unix)]
use nix::{
    cmsg_space,
    sys::{
        socket::{recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags},
        uio::IoVec,
    },
};
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, RawFd};

#[cfg(all(unix, feature = "async-io"))]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use crate::{utils::FDS_MAX, OwnedFd};

#[cfg(unix)]
fn fd_recvmsg(fd: RawFd, buffer: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    let iov = [IoVec::from_mut_slice(buffer)];
    let mut cmsgspace = cmsg_space!([RawFd; FDS_MAX]);

    match recvmsg(fd, &iov, Some(&mut cmsgspace), MsgFlags::empty()) {
        Ok(msg) => {
            let mut fds = vec![];
            for cmsg in msg.cmsgs() {
                #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
                if let ControlMessageOwned::ScmCreds(_) = cmsg {
                    continue;
                }
                if let ControlMessageOwned::ScmRights(fd) = cmsg {
                    fds.extend(fd.iter().map(|&f| unsafe { OwnedFd::from_raw_fd(f) }));
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected CMSG kind",
                    ));
                }
            }
            Ok((msg.bytes, fds))
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
fn fd_sendmsg(fd: RawFd, buffer: &[u8], fds: &[RawFd]) -> io::Result<usize> {
    let cmsg = if !fds.is_empty() {
        vec![ControlMessage::ScmRights(fds)]
    } else {
        vec![]
    };
    let iov = [IoVec::from_slice(buffer)];
    match sendmsg(fd, &iov, &cmsg, MsgFlags::empty(), None) {
        // can it really happen?
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "failed to write to buffer",
        )),
        Ok(n) => Ok(n),
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
type PollRecvmsg = Poll<io::Result<(usize, Vec<OwnedFd>)>>;

#[cfg(not(unix))]
type PollRecvmsg = Poll<io::Result<usize>>;

/// Trait representing some transport layer over which the DBus protocol can be used
///
/// The crate provides implementations for `async_io` and `tokio`'s `UnixStream` wrappers if you
/// enable the corresponding crate features (`async_io` is enabled by default).
///
/// You can implement it manually to integrate with other runtimes or other dbus transports.  Feel
/// free to submit pull requests to add support for more runtimes to zbus itself so rust's orphan
/// rules don't force the use of a wrapper struct (and to avoid duplicating the work across many
/// projects).
pub trait Socket: std::fmt::Debug + Send + Sync {
    /// Supports passing file descriptors.
    fn can_pass_unix_fd(&self) -> bool {
        true
    }

    /// Attempt to receive a message from the socket.
    ///
    /// On success, returns the number of bytes read as well as a `Vec` containing
    /// any associated file descriptors.
    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg;

    /// Attempt to send a message on the socket
    ///
    /// On success, return the number of bytes written. There may be a partial write, in
    /// which case the caller is responsible of sending the remaining data by calling this
    /// method again until everything is written or it returns an error of kind `WouldBlock`.
    ///
    /// If at least one byte has been written, then all the provided file descriptors will
    /// have been sent as well, and should not be provided again in subsequent calls.
    ///
    /// If the underlying transport does not support transmitting file descriptors, this
    /// will return `Err(ErrorKind::InvalidInput)`.
    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>>;

    /// Close the socket.
    ///
    /// After this call, it is valid for all reading and writing operations to fail.
    fn close(&self) -> io::Result<()>;

    /// Return the raw file descriptor backing this transport, if any.
    ///
    /// This is used to back some internal platform-specific functions.
    #[cfg(unix)]
    fn as_raw_fd(&self) -> RawFd;

    /// Return the peer process SID, if any.
    #[cfg(windows)]
    fn peer_sid(&self) -> Option<String> {
        None
    }
}

impl Socket for Box<dyn Socket> {
    fn can_pass_unix_fd(&self) -> bool {
        (&**self).can_pass_unix_fd()
    }

    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        (&mut **self).poll_recvmsg(cx, buf)
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>> {
        (&mut **self).poll_sendmsg(
            cx,
            buffer,
            #[cfg(unix)]
            fds,
        )
    }

    fn close(&self) -> io::Result<()> {
        (&**self).close()
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> RawFd {
        (&**self).as_raw_fd()
    }

    #[cfg(windows)]
    fn peer_sid(&self) -> Option<String> {
        (&**self).peer_sid()
    }
}

#[cfg(all(unix, feature = "async-io"))]
impl Socket for Async<UnixStream> {
    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        let (len, fds) = loop {
            match fd_recvmsg(self.as_raw_fd(), buf) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => match self.poll_readable(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res?,
                },
                v => break v?,
            }
        };
        Poll::Ready(Ok((len, fds)))
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>> {
        loop {
            match fd_sendmsg(
                self.as_raw_fd(),
                buffer,
                #[cfg(unix)]
                fds,
            ) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => match self.poll_writable(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res?,
                },
                v => return Poll::Ready(v),
            }
        }
    }

    fn close(&self) -> io::Result<()> {
        self.get_ref().shutdown(std::net::Shutdown::Both)
    }

    fn as_raw_fd(&self) -> RawFd {
        // This causes a name collision if imported
        std::os::unix::io::AsRawFd::as_raw_fd(self.get_ref())
    }
}

#[cfg(all(unix, feature = "tokio"))]
impl Socket for tokio::net::UnixStream {
    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        loop {
            match self.try_io(tokio::io::Interest::READABLE, || {
                fd_recvmsg(self.as_raw_fd(), buf)
            }) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => match self.poll_read_ready(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res?,
                },
                v => return Poll::Ready(v),
            }
        }
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>> {
        loop {
            match self.try_io(tokio::io::Interest::WRITABLE, || {
                fd_sendmsg(
                    self.as_raw_fd(),
                    buffer,
                    #[cfg(unix)]
                    fds,
                )
            }) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    match self.poll_write_ready(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(res) => res?,
                    }
                }
                v => return Poll::Ready(v),
            }
        }
    }

    fn close(&self) -> io::Result<()> {
        Ok(())
    }

    fn as_raw_fd(&self) -> RawFd {
        // This causes a name collision if imported
        std::os::unix::io::AsRawFd::as_raw_fd(self)
    }
}

#[cfg(all(windows, feature = "async-io"))]
impl Socket for Async<UnixStream> {
    fn can_pass_unix_fd(&self) -> bool {
        false
    }

    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        loop {
            match (&mut *self).get_mut().read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
                Ok(len) => {
                    let ret = len;
                    return Poll::Ready(Ok(ret));
                }
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_sendmsg(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            match (&mut *self).get_mut().write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn close(&self) -> io::Result<()> {
        self.get_ref().shutdown(std::net::Shutdown::Both)
    }

    #[cfg(windows)]
    fn peer_sid(&self) -> Option<String> {
        use crate::win32::{unix_stream_get_peer_pid, ProcessToken};

        if let Ok(pid) = unix_stream_get_peer_pid(&self.get_ref()) {
            if let Ok(process_token) = ProcessToken::open(if pid != 0 { Some(pid) } else { None }) {
                return process_token.sid().ok();
            }
        }

        None
    }
}

#[cfg(feature = "async-io")]
impl Socket for Async<TcpStream> {
    fn can_pass_unix_fd(&self) -> bool {
        false
    }

    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        #[cfg(unix)]
        let fds = vec![];

        loop {
            match (&mut *self).get_mut().read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
                Ok(len) => {
                    #[cfg(unix)]
                    let ret = (len, fds);
                    #[cfg(not(unix))]
                    let ret = len;
                    return Poll::Ready(Ok(ret));
                }
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>> {
        #[cfg(unix)]
        if !fds.is_empty() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fds cannot be sent with a tcp stream",
            )));
        }

        loop {
            match (&mut *self).get_mut().write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn close(&self) -> io::Result<()> {
        self.get_ref().shutdown(std::net::Shutdown::Both)
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> RawFd {
        // This causes a name collision if imported
        std::os::unix::io::AsRawFd::as_raw_fd(self.get_ref())
    }

    #[cfg(windows)]
    fn peer_sid(&self) -> Option<String> {
        use crate::win32::{tcp_stream_get_peer_pid, ProcessToken};

        if let Ok(pid) = tcp_stream_get_peer_pid(&self.get_ref()) {
            if let Ok(process_token) = ProcessToken::open(if pid != 0 { Some(pid) } else { None }) {
                return process_token.sid().ok();
            }
        }

        None
    }
}

#[cfg(feature = "tokio")]
impl Socket for tokio::net::TcpStream {
    fn can_pass_unix_fd(&self) -> bool {
        false
    }

    fn poll_recvmsg(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> PollRecvmsg {
        #[cfg(unix)]
        let fds = vec![];

        loop {
            match self.try_read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Poll::Ready(Err(e)),
                Ok(len) => {
                    #[cfg(unix)]
                    let ret = (len, fds);
                    #[cfg(not(unix))]
                    let ret = len;
                    return Poll::Ready(Ok(ret));
                }
            }
            ready!(self.poll_read_ready(cx))?;
        }
    }

    fn poll_sendmsg(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        #[cfg(unix)] fds: &[RawFd],
    ) -> Poll<io::Result<usize>> {
        #[cfg(unix)]
        if !fds.is_empty() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "fds cannot be sent with a tcp stream",
            )));
        }

        loop {
            match self.try_write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_write_ready(cx))?;
        }
    }

    fn close(&self) -> io::Result<()> {
        Ok(())
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> RawFd {
        // This causes a name collision if imported
        std::os::unix::io::AsRawFd::as_raw_fd(self)
    }

    #[cfg(windows)]
    fn peer_sid(&self) -> Option<String> {
        use crate::win32::{socket_addr_get_pid, ProcessToken};

        let peer_addr = match self.peer_addr() {
            Ok(addr) => addr,
            Err(_) => return None,
        };

        if let Ok(pid) = socket_addr_get_pid(&peer_addr) {
            if let Ok(process_token) = ProcessToken::open(if pid != 0 { Some(pid) } else { None }) {
                return process_token.sid().ok();
            }
        }

        None
    }
}
