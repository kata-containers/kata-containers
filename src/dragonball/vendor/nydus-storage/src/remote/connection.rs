// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Structs for Unix Domain Socket listener and endpoint.
//!
//! This file is copied from vhost/src/vhost-user/connection.rs, please keep it as is when possible.

#![allow(dead_code)]

use std::fs::File;
use std::io::Error as IOError;
use std::io::ErrorKind;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::{mem, slice};

use libc::{c_void, iovec};
use vm_memory::ByteValued;

use super::message::*;
use dbs_uhttp::{ScmSocket, SysError};
use std::net::Shutdown;

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum Error {
    /// Invalid parameters.
    InvalidParam,
    /// Unsupported operations due to that the protocol feature hasn't been negotiated.
    InvalidOperation,
    /// Invalid message format, flag or content.
    InvalidMessage,
    /// Only part of a message have been sent or received successfully
    PartialMessage,
    /// Message is too large
    OversizedMsg,
    /// Fd array in question is too big or too small
    IncorrectFds,
    /// Can't connect to peer.
    SocketConnect(std::io::Error),
    /// Generic socket errors.
    SocketError(std::io::Error),
    /// The socket is broken or has been closed.
    SocketBroken(std::io::Error),
    /// Should retry the socket operation again.
    SocketRetry(std::io::Error),
    /// Failure from the slave side.
    SlaveInternalError,
    /// Failure from the master side.
    MasterInternalError,
    /// Virtio/protocol features mismatch.
    FeatureMismatch,
    /// Error from request handler
    ReqHandlerError(IOError),
    /// memfd file creation error
    MemFdCreateError,
    /// File truncate error
    FileTrucateError,
    /// memfd file seal errors
    MemFdSealError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::InvalidParam => write!(f, "invalid parameters"),
            Error::InvalidOperation => write!(f, "invalid operation"),
            Error::InvalidMessage => write!(f, "invalid message"),
            Error::PartialMessage => write!(f, "partial message"),
            Error::OversizedMsg => write!(f, "oversized message"),
            Error::IncorrectFds => write!(f, "wrong number of attached fds"),
            Error::SocketError(e) => write!(f, "socket error: {}", e),
            Error::SocketConnect(e) => write!(f, "can't connect to peer: {}", e),
            Error::SocketBroken(e) => write!(f, "socket is broken: {}", e),
            Error::SocketRetry(e) => write!(f, "temporary socket error: {}", e),
            Error::SlaveInternalError => write!(f, "slave internal error"),
            Error::MasterInternalError => write!(f, "Master internal error"),
            Error::FeatureMismatch => write!(f, "virtio/protocol features mismatch"),
            Error::ReqHandlerError(e) => write!(f, "handler failed to handle request: {}", e),
            Error::MemFdCreateError => {
                write!(f, "handler failed to allocate memfd during get_inflight_fd")
            }
            Error::FileTrucateError => {
                write!(f, "handler failed to trucate memfd during get_inflight_fd")
            }
            Error::MemFdSealError => write!(
                f,
                "handler failed to apply seals to memfd during get_inflight_fd"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Determine whether to rebuild the underline communication channel.
    pub fn should_reconnect(&self) -> bool {
        match *self {
            // Should reconnect because it may be caused by temporary network errors.
            Error::PartialMessage => true,
            // Should reconnect because the underline socket is broken.
            Error::SocketBroken(_) => true,
            // Slave internal error, hope it recovers on reconnect.
            Error::SlaveInternalError => true,
            // Master internal error, hope it recovers on reconnect.
            Error::MasterInternalError => true,
            // Should just retry the IO operation instead of rebuilding the underline connection.
            Error::SocketRetry(_) => false,
            Error::InvalidParam | Error::InvalidOperation => false,
            Error::InvalidMessage | Error::IncorrectFds | Error::OversizedMsg => false,
            Error::SocketError(_) | Error::SocketConnect(_) => false,
            Error::FeatureMismatch => false,
            Error::ReqHandlerError(_) => false,
            Error::MemFdCreateError | Error::FileTrucateError | Error::MemFdSealError => false,
        }
    }
}

impl std::convert::From<std::io::Error> for Error {
    #[allow(unreachable_patterns)] // EWOULDBLOCK equals to EGAIN on linux
    fn from(err: std::io::Error) -> Self {
        Error::SocketError(err)
    }
}

impl std::convert::From<SysError> for Error {
    /// Convert raw socket errors into meaningful blob manager errors.
    ///
    /// The vmm_sys_util::errno::Error is a simple wrapper over the raw errno, which doesn't means
    /// much to the connection manager. So convert it into meaningful errors to simplify
    /// the connection manager logic.
    ///
    /// # Return:
    /// * - Error::SocketRetry: temporary error caused by signals or short of resources.
    /// * - Error::SocketBroken: the underline socket is broken.
    /// * - Error::SocketError: other socket related errors.
    #[allow(unreachable_patterns)] // EWOULDBLOCK equals to EGAIN on linux
    fn from(err: SysError) -> Self {
        match err.errno() {
            // The socket is marked nonblocking and the requested operation would block.
            libc::EAGAIN => Error::SocketRetry(IOError::from_raw_os_error(libc::EAGAIN)),
            // The socket is marked nonblocking and the requested operation would block.
            libc::EWOULDBLOCK => Error::SocketRetry(IOError::from_raw_os_error(libc::EWOULDBLOCK)),
            // A signal occurred before any data was transmitted
            libc::EINTR => Error::SocketRetry(IOError::from_raw_os_error(libc::EINTR)),
            // The  output  queue  for  a network interface was full.  This generally indicates
            // that the interface has stopped sending, but may be caused by transient congestion.
            libc::ENOBUFS => Error::SocketRetry(IOError::from_raw_os_error(libc::ENOBUFS)),
            // No memory available.
            libc::ENOMEM => Error::SocketRetry(IOError::from_raw_os_error(libc::ENOMEM)),
            // Connection reset by peer.
            libc::ECONNRESET => Error::SocketBroken(IOError::from_raw_os_error(libc::ECONNRESET)),
            // The local end has been shut down on a connection oriented socket. In this  case the
            // process will also receive a SIGPIPE unless MSG_NOSIGNAL is set.
            libc::EPIPE => Error::SocketBroken(IOError::from_raw_os_error(libc::EPIPE)),
            // Write permission is denied on the destination socket file, or search permission is
            // denied for one of the directories the path prefix.
            libc::EACCES => Error::SocketConnect(IOError::from_raw_os_error(libc::EACCES)),
            // Catch all other errors
            e => Error::SocketError(IOError::from_raw_os_error(e)),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Unix domain socket listener for accepting incoming connections.
pub(crate) struct Listener {
    fd: UnixListener,
    path: Option<PathBuf>,
}

impl Listener {
    /// Create a unix domain socket listener.
    ///
    /// # Return:
    /// * - the new Listener object on success.
    /// * - SocketError: failed to create listener socket.
    pub fn new<P: AsRef<Path>>(path: P, unlink: bool) -> Result<Self> {
        if unlink {
            let _ = std::fs::remove_file(&path);
        }
        let fd = UnixListener::bind(&path).map_err(Error::SocketError)?;

        Ok(Listener {
            fd,
            path: Some(path.as_ref().to_owned()),
        })
    }

    /// Accept an incoming connection.
    ///
    /// # Return:
    /// * - Some(UnixStream): new UnixStream object if new incoming connection is available.
    /// * - None: no incoming connection available.
    /// * - SocketError: errors from accept().
    pub fn accept(&self) -> Result<Option<UnixStream>> {
        loop {
            match self.fd.accept() {
                Ok((socket, _addr)) => return Ok(Some(socket)),
                Err(e) => {
                    match e.kind() {
                        // No incoming connection available.
                        ErrorKind::WouldBlock => return Ok(None),
                        // New connection closed by peer.
                        ErrorKind::ConnectionAborted => return Ok(None),
                        // Interrupted by signals, retry
                        ErrorKind::Interrupted => continue,
                        _ => return Err(Error::SocketError(e)),
                    }
                }
            }
        }
    }

    /// Change blocking status on the listener.
    ///
    /// # Return:
    /// * - () on success.
    /// * - SocketError: failure from set_nonblocking().
    pub fn set_nonblocking(&self, block: bool) -> Result<()> {
        self.fd.set_nonblocking(block).map_err(Error::SocketError)
    }
}

impl AsRawFd for Listener {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl FromRawFd for Listener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Listener {
            fd: UnixListener::from_raw_fd(fd),
            path: None,
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Unix domain socket endpoint.
pub(crate) struct Endpoint {
    sock: UnixStream,
}

impl Endpoint {
    /// Create a new stream by connecting to server at `str`.
    ///
    /// # Return:
    /// * - the new Endpoint object on success.
    /// * - SocketConnect: failed to connect to peer.
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let sock = UnixStream::connect(path).map_err(Error::SocketConnect)?;
        Ok(Self::from_stream(sock))
    }

    /// Create an endpoint from a stream object.
    pub fn from_stream(sock: UnixStream) -> Self {
        Endpoint { sock }
    }

    /// Close the underlying socket.
    pub fn close(&self) {
        let _ = self.sock.shutdown(Shutdown::Both);
    }

    /// Sends bytes from scatter-gather vectors over the socket with optional attached file
    /// descriptors.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn send_iovec(&mut self, iovs: &[&[u8]], fds: Option<&[RawFd]>) -> Result<usize> {
        let rfds = match fds {
            Some(rfds) => rfds,
            _ => &[],
        };
        self.sock.send_with_fds(iovs, rfds).map_err(Into::into)
    }

    /// Sends all bytes from scatter-gather vectors over the socket with optional attached file
    /// descriptors. Will loop until all data has been transfered.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn send_iovec_all(&mut self, iovs: &[&[u8]], fds: Option<&[RawFd]>) -> Result<usize> {
        let mut data_sent = 0;
        let mut data_total = 0;
        let iov_lens: Vec<usize> = iovs.iter().map(|iov| iov.len()).collect();
        for len in &iov_lens {
            data_total += len;
        }

        while (data_total - data_sent) > 0 {
            let (nr_skip, offset) = get_sub_iovs_offset(&iov_lens, data_sent);
            let iov = &iovs[nr_skip][offset..];

            let data = &[&[iov], &iovs[(nr_skip + 1)..]].concat();
            let sfds = if data_sent == 0 { fds } else { None };

            let sent = self.send_iovec(data, sfds);
            match sent {
                Ok(0) => return Ok(data_sent),
                Ok(n) => data_sent += n,
                Err(e) => match e {
                    Error::SocketRetry(_) => {}
                    _ => return Err(e),
                },
            }
        }
        Ok(data_sent)
    }

    /// Sends bytes from a slice over the socket with optional attached file descriptors.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn send_slice(&mut self, data: &[u8], fds: Option<&[RawFd]>) -> Result<usize> {
        self.send_iovec(&[data], fds)
    }

    /// Sends a header-only message with optional attached file descriptors.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    pub fn send_header(&mut self, hdr: &MsgHeader, fds: Option<&[RawFd]>) -> Result<()> {
        // Safe because there can't be other mutable referance to hdr.
        let iovs = unsafe {
            [slice::from_raw_parts(
                hdr as *const MsgHeader as *const u8,
                mem::size_of::<MsgHeader>(),
            )]
        };
        let bytes = self.send_iovec_all(&iovs[..], fds)?;
        if bytes != mem::size_of::<MsgHeader>() {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Send a message with header and body. Optional file descriptors may be attached to
    /// the message.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    pub fn send_message<T: Sized>(
        &mut self,
        hdr: &MsgHeader,
        body: &T,
        fds: Option<&[RawFd]>,
    ) -> Result<()> {
        if mem::size_of::<T>() > MAX_MSG_SIZE {
            return Err(Error::OversizedMsg);
        }
        // Safe because there can't be other mutable referance to hdr and body.
        let iovs = unsafe {
            [
                slice::from_raw_parts(
                    hdr as *const MsgHeader as *const u8,
                    mem::size_of::<MsgHeader>(),
                ),
                slice::from_raw_parts(body as *const T as *const u8, mem::size_of::<T>()),
            ]
        };
        let bytes = self.send_iovec_all(&iovs[..], fds)?;
        if bytes != mem::size_of::<MsgHeader>() + mem::size_of::<T>() {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Send a message with header, body and payload. Optional file descriptors
    /// may also be attached to the message.
    ///
    /// # Return:
    /// * - number of bytes sent on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - OversizedMsg: message size is too big.
    /// * - PartialMessage: received a partial message.
    /// * - IncorrectFds: wrong number of attached fds.
    pub fn send_message_with_payload<T: Sized>(
        &mut self,
        hdr: &MsgHeader,
        body: &T,
        payload: &[u8],
        fds: Option<&[RawFd]>,
    ) -> Result<()> {
        let len = payload.len();
        if mem::size_of::<T>() > MAX_MSG_SIZE {
            return Err(Error::OversizedMsg);
        }
        if len > MAX_MSG_SIZE - mem::size_of::<T>() {
            return Err(Error::OversizedMsg);
        }
        if let Some(fd_arr) = fds {
            if fd_arr.len() > MAX_ATTACHED_FD_ENTRIES {
                return Err(Error::IncorrectFds);
            }
        }

        // Safe because there can't be other mutable reference to hdr, body and payload.
        let iovs = unsafe {
            [
                slice::from_raw_parts(
                    hdr as *const MsgHeader as *const u8,
                    mem::size_of::<MsgHeader>(),
                ),
                slice::from_raw_parts(body as *const T as *const u8, mem::size_of::<T>()),
                slice::from_raw_parts(payload.as_ptr() as *const u8, len),
            ]
        };
        let total = mem::size_of::<MsgHeader>() + mem::size_of::<T>() + len;
        let len = self.send_iovec_all(&iovs, fds)?;
        if len != total {
            return Err(Error::PartialMessage);
        }
        Ok(())
    }

    /// Reads bytes from the socket into the given scatter/gather vectors.
    ///
    /// # Return:
    /// * - (number of bytes received, buf) on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn recv_data(&mut self, len: usize) -> Result<(usize, Vec<u8>)> {
        let mut rbuf = vec![0u8; len];
        let mut iovs = [iovec {
            iov_base: rbuf.as_mut_ptr() as *mut c_void,
            iov_len: len,
        }];
        // Safe because we own rbuf and it's safe to fill a byte array with arbitrary data.
        let (bytes, _) = unsafe { self.sock.recv_with_fds(&mut iovs, &mut [])? };
        Ok((bytes, rbuf))
    }

    /// Reads bytes from the socket into the given scatter/gather vectors with optional attached
    /// file.
    ///
    /// The underlying communication channel is a Unix domain socket in STREAM mode. It's a little
    /// tricky to pass file descriptors through such a communication channel. Let's assume that a
    /// sender sending a message with some file descriptors attached. To successfully receive those
    /// attached file descriptors, the receiver must obey following rules:
    ///   1) file descriptors are attached to a message.
    ///   2) message(packet) boundaries must be respected on the receive side.
    /// In other words, recvmsg() operations must not cross the packet boundary, otherwise the
    /// attached file descriptors will get lost.
    /// Note that this function wraps received file descriptors as `File`.
    ///
    /// # Return:
    /// * - (number of bytes received, [received files]) on success
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to ensure it is safe for arbitrary data to be
    /// written to the iovec pointers.
    pub unsafe fn recv_into_iovec(
        &mut self,
        iovs: &mut [iovec],
    ) -> Result<(usize, Option<Vec<File>>)> {
        let mut fd_array = vec![0; MAX_ATTACHED_FD_ENTRIES];

        let (bytes, fds) = self.sock.recv_with_fds(iovs, &mut fd_array)?;

        let files = match fds {
            0 => None,
            n => {
                let files = fd_array
                    .iter()
                    .take(n)
                    .map(|fd| {
                        // Safe because we have the ownership of `fd`.
                        File::from_raw_fd(*fd)
                    })
                    .collect();
                Some(files)
            }
        };

        Ok((bytes, files))
    }

    /// Reads all bytes from the socket into the given scatter/gather vectors with optional
    /// attached files. Will loop until all data has been transferred.
    ///
    /// The underlying communication channel is a Unix domain socket in STREAM mode. It's a little
    /// tricky to pass file descriptors through such a communication channel. Let's assume that a
    /// sender sending a message with some file descriptors attached. To successfully receive those
    /// attached file descriptors, the receiver must obey following rules:
    ///   1) file descriptors are attached to a message.
    ///   2) message(packet) boundaries must be respected on the receive side.
    /// In other words, recvmsg() operations must not cross the packet boundary, otherwise the
    /// attached file descriptors will get lost.
    /// Note that this function wraps received file descriptors as `File`.
    ///
    /// # Return:
    /// * - (number of bytes received, [received fds]) on success
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to ensure it is safe for arbitrary data to be
    /// written to the iovec pointers.
    pub unsafe fn recv_into_iovec_all(
        &mut self,
        iovs: &mut [iovec],
    ) -> Result<(usize, Option<Vec<File>>)> {
        let mut data_read = 0;
        let mut data_total = 0;
        let mut rfds = None;
        let iov_lens: Vec<usize> = iovs.iter().map(|iov| iov.iov_len).collect();
        for len in &iov_lens {
            data_total += len;
        }

        while (data_total - data_read) > 0 {
            let (nr_skip, offset) = get_sub_iovs_offset(&iov_lens, data_read);
            let iov = &mut iovs[nr_skip];

            let mut data = [
                &[iovec {
                    iov_base: (iov.iov_base as usize + offset) as *mut c_void,
                    iov_len: iov.iov_len - offset,
                }],
                &iovs[(nr_skip + 1)..],
            ]
            .concat();

            let res = self.recv_into_iovec(&mut data);
            match res {
                Ok((0, _)) => return Ok((data_read, rfds)),
                Ok((n, fds)) => {
                    if data_read == 0 {
                        rfds = fds;
                    }
                    data_read += n;
                }
                Err(e) => match e {
                    Error::SocketRetry(_) => {}
                    _ => return Err(e),
                },
            }
        }
        Ok((data_read, rfds))
    }

    /// Reads bytes from the socket into a new buffer with optional attached
    /// files. Received file descriptors are set close-on-exec and converted to `File`.
    ///
    /// # Return:
    /// * - (number of bytes received, buf, [received files]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    pub fn recv_into_buf(
        &mut self,
        buf_size: usize,
    ) -> Result<(usize, Vec<u8>, Option<Vec<File>>)> {
        let mut buf = vec![0u8; buf_size];
        let (bytes, files) = {
            let mut iovs = [iovec {
                iov_base: buf.as_mut_ptr() as *mut c_void,
                iov_len: buf_size,
            }];
            // Safe because we own buf and it's safe to fill a byte array with arbitrary data.
            unsafe { self.recv_into_iovec(&mut iovs)? }
        };
        Ok((bytes, buf, files))
    }

    /// Receive a header-only message with optional attached files.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, [received files]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    pub fn recv_header(&mut self) -> Result<(MsgHeader, Option<Vec<File>>)> {
        let mut hdr = MsgHeader::default();
        let mut iovs = [iovec {
            iov_base: (&mut hdr as *mut MsgHeader) as *mut c_void,
            iov_len: mem::size_of::<MsgHeader>(),
        }];
        // Safe because we own hdr and it's ByteValued.
        let (bytes, files) = unsafe { self.recv_into_iovec_all(&mut iovs[..])? };

        if bytes != mem::size_of::<MsgHeader>() {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, files))
    }

    /// Receive a message with optional attached file descriptors.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, message body, [received files]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    pub fn recv_body<T: ByteValued + Sized + MsgValidator>(
        &mut self,
    ) -> Result<(MsgHeader, T, Option<Vec<File>>)> {
        let mut hdr = MsgHeader::default();
        let mut body: T = Default::default();
        let mut iovs = [
            iovec {
                iov_base: (&mut hdr as *mut MsgHeader) as *mut c_void,
                iov_len: mem::size_of::<MsgHeader>(),
            },
            iovec {
                iov_base: (&mut body as *mut T) as *mut c_void,
                iov_len: mem::size_of::<T>(),
            },
        ];
        // Safe because we own hdr and body and they're ByteValued.
        let (bytes, files) = unsafe { self.recv_into_iovec_all(&mut iovs[..])? };

        let total = mem::size_of::<MsgHeader>() + mem::size_of::<T>();
        if bytes != total {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() || !body.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, body, files))
    }

    /// Receive a message with header and optional content. Callers need to
    /// pre-allocate a big enough buffer to receive the message body and
    /// optional payload. If there are attached file descriptor associated
    /// with the message, the first MAX_ATTACHED_FD_ENTRIES file descriptors
    /// will be accepted and all other file descriptor will be discard
    /// silently.
    ///
    /// # Return:
    /// * - (message header, message size, [received files]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    pub fn recv_body_into_buf(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(MsgHeader, usize, Option<Vec<File>>)> {
        let mut hdr = MsgHeader::default();
        let mut iovs = [
            iovec {
                iov_base: (&mut hdr as *mut MsgHeader) as *mut c_void,
                iov_len: mem::size_of::<MsgHeader>(),
            },
            iovec {
                iov_base: buf.as_mut_ptr() as *mut c_void,
                iov_len: buf.len(),
            },
        ];
        // Safe because we own hdr and have a mutable borrow of buf, and hdr is ByteValued
        // and it's safe to fill a byte slice with arbitrary data.
        let (bytes, files) = unsafe { self.recv_into_iovec_all(&mut iovs[..])? };

        if bytes < mem::size_of::<MsgHeader>() {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, bytes - mem::size_of::<MsgHeader>(), files))
    }

    /// Receive a message with optional payload and attached file descriptors.
    /// Note, only the first MAX_ATTACHED_FD_ENTRIES file descriptors will be
    /// accepted and all other file descriptor will be discard silently.
    ///
    /// # Return:
    /// * - (message header, message body, size of payload, [received files]) on success.
    /// * - SocketRetry: temporary error caused by signals or short of resources.
    /// * - SocketBroken: the underline socket is broken.
    /// * - SocketError: other socket related errors.
    /// * - PartialMessage: received a partial message.
    /// * - InvalidMessage: received a invalid message.
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::type_complexity))]
    pub fn recv_payload_into_buf<T: ByteValued + Sized + MsgValidator>(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(MsgHeader, T, usize, Option<Vec<File>>)> {
        let mut hdr = MsgHeader::default();
        let mut body: T = Default::default();
        let mut iovs = [
            iovec {
                iov_base: (&mut hdr as *mut MsgHeader) as *mut c_void,
                iov_len: mem::size_of::<MsgHeader>(),
            },
            iovec {
                iov_base: (&mut body as *mut T) as *mut c_void,
                iov_len: mem::size_of::<T>(),
            },
            iovec {
                iov_base: buf.as_mut_ptr() as *mut c_void,
                iov_len: buf.len(),
            },
        ];
        // Safe because we own hdr and body and have a mutable borrow of buf, and
        // hdr and body are ByteValued, and it's safe to fill a byte slice with
        // arbitrary data.
        let (bytes, files) = unsafe { self.recv_into_iovec_all(&mut iovs[..])? };

        let total = mem::size_of::<MsgHeader>() + mem::size_of::<T>();
        if bytes < total {
            return Err(Error::PartialMessage);
        } else if !hdr.is_valid() || !body.is_valid() {
            return Err(Error::InvalidMessage);
        }

        Ok((hdr, body, bytes - total, files))
    }
}

impl AsRawFd for Endpoint {
    fn as_raw_fd(&self) -> RawFd {
        self.sock.as_raw_fd()
    }
}

// Given a slice of sizes and the `skip_size`, return the offset of `skip_size` in the slice.
// For example:
//     let iov_lens = vec![4, 4, 5];
//     let size = 6;
//     assert_eq!(get_sub_iovs_offset(&iov_len, size), (1, 2));
fn get_sub_iovs_offset(iov_lens: &[usize], skip_size: usize) -> (usize, usize) {
    let mut size = skip_size;
    let mut nr_skip = 0;

    for len in iov_lens {
        if size >= *len {
            size -= *len;
            nr_skip += 1;
        } else {
            break;
        }
    }
    (nr_skip, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use vmm_sys_util::rand::rand_alphanumerics;
    use vmm_sys_util::tempfile::TempFile;

    fn temp_path() -> PathBuf {
        PathBuf::from(format!(
            "/tmp/blob_test_{}",
            rand_alphanumerics(8).to_str().unwrap()
        ))
    }

    #[test]
    fn create_listener() {
        let path = temp_path();
        let listener = Listener::new(&path, true).unwrap();

        assert!(listener.as_raw_fd() > 0);
    }

    #[test]
    fn create_listener_from_raw_fd() {
        let path = temp_path();
        let file = File::create(path).unwrap();
        let listener = unsafe { Listener::from_raw_fd(file.as_raw_fd()) };

        assert!(listener.as_raw_fd() > 0);
    }

    #[test]
    fn accept_connection() {
        let path = temp_path();
        let listener = Listener::new(&path, true).unwrap();
        listener.set_nonblocking(true).unwrap();

        // accept on a fd without incoming connection
        let conn = listener.accept().unwrap();
        assert!(conn.is_none());
    }

    #[test]
    fn send_data() {
        let path = temp_path();
        let listener = Listener::new(&path, true).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut master = Endpoint::connect(&path).unwrap();
        let sock = listener.accept().unwrap().unwrap();
        let mut slave = Endpoint::from_stream(sock);

        let buf1 = vec![0x1, 0x2, 0x3, 0x4];
        let mut len = master.send_slice(&buf1[..], None).unwrap();
        assert_eq!(len, 4);
        let (bytes, buf2, _) = slave.recv_into_buf(0x1000).unwrap();
        assert_eq!(bytes, 4);
        assert_eq!(&buf1[..], &buf2[..bytes]);

        len = master.send_slice(&buf1[..], None).unwrap();
        assert_eq!(len, 4);
        let (bytes, buf2, _) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[..2], &buf2[..]);
        let (bytes, buf2, _) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[2..], &buf2[..]);
    }

    #[test]
    fn send_fd() {
        let path = temp_path();
        let listener = Listener::new(&path, true).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut master = Endpoint::connect(&path).unwrap();
        let sock = listener.accept().unwrap().unwrap();
        let mut slave = Endpoint::from_stream(sock);

        let mut fd = TempFile::new().unwrap().into_file();
        write!(fd, "test").unwrap();

        // Normal case for sending/receiving file descriptors
        let buf1 = vec![0x1, 0x2, 0x3, 0x4];
        let len = master
            .send_slice(&buf1[..], Some(&[fd.as_raw_fd()]))
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, buf2, files) = slave.recv_into_buf(4).unwrap();
        assert_eq!(bytes, 4);
        assert_eq!(&buf1[..], &buf2[..]);
        assert!(files.is_some());
        let files = files.unwrap();
        {
            assert_eq!(files.len(), 1);
            let mut file = &files[0];
            let mut content = String::new();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.read_to_string(&mut content).unwrap();
            assert_eq!(content, "test");
        }

        // Following communication pattern should work:
        // Sending side: data(header, body) with fds
        // Receiving side: data(header) with fds, data(body)
        let len = master
            .send_slice(
                &buf1[..],
                Some(&[fd.as_raw_fd(), fd.as_raw_fd(), fd.as_raw_fd()]),
            )
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, buf2, files) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[..2], &buf2[..]);
        assert!(files.is_some());
        let files = files.unwrap();
        {
            assert_eq!(files.len(), 3);
            let mut file = &files[1];
            let mut content = String::new();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.read_to_string(&mut content).unwrap();
            assert_eq!(content, "test");
        }
        let (bytes, buf2, files) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[2..], &buf2[..]);
        assert!(files.is_none());

        // Following communication pattern should not work:
        // Sending side: data(header, body) with fds
        // Receiving side: data(header), data(body) with fds
        let len = master
            .send_slice(
                &buf1[..],
                Some(&[fd.as_raw_fd(), fd.as_raw_fd(), fd.as_raw_fd()]),
            )
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, buf4) = slave.recv_data(2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[..2], &buf4[..]);
        let (bytes, buf2, files) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[2..], &buf2[..]);
        assert!(files.is_none());

        // Following communication pattern should work:
        // Sending side: data, data with fds
        // Receiving side: data, data with fds
        let len = master.send_slice(&buf1[..], None).unwrap();
        assert_eq!(len, 4);
        let len = master
            .send_slice(
                &buf1[..],
                Some(&[fd.as_raw_fd(), fd.as_raw_fd(), fd.as_raw_fd()]),
            )
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, buf2, files) = slave.recv_into_buf(0x4).unwrap();
        assert_eq!(bytes, 4);
        assert_eq!(&buf1[..], &buf2[..]);
        assert!(files.is_none());

        let (bytes, buf2, files) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[..2], &buf2[..]);
        assert!(files.is_some());
        let files = files.unwrap();
        {
            assert_eq!(files.len(), 3);
            let mut file = &files[1];
            let mut content = String::new();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.read_to_string(&mut content).unwrap();
            assert_eq!(content, "test");
        }
        let (bytes, buf2, files) = slave.recv_into_buf(0x2).unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(&buf1[2..], &buf2[..]);
        assert!(files.is_none());

        // Following communication pattern should not work:
        // Sending side: data1, data2 with fds
        // Receiving side: data + partial of data2, left of data2 with fds
        let len = master.send_slice(&buf1[..], None).unwrap();
        assert_eq!(len, 4);
        let len = master
            .send_slice(
                &buf1[..],
                Some(&[fd.as_raw_fd(), fd.as_raw_fd(), fd.as_raw_fd()]),
            )
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, _buf) = slave.recv_data(5).unwrap();
        #[cfg(target_os = "linux")]
        assert_eq!(bytes, 5);

        #[cfg(target_os = "macos")]
        assert_eq!(bytes, 4);

        let (bytes, _buf, files) = slave.recv_into_buf(0x4).unwrap();
        #[cfg(target_os = "linux")]
        assert_eq!(bytes, 3);
        #[cfg(target_os = "linux")]
        assert!(files.is_none());

        #[cfg(target_os = "macos")]
        assert_eq!(bytes, 4);
        #[cfg(target_os = "macos")]
        assert!(files.is_some());

        // If the target fd array is too small, extra file descriptors will get lost.
        let len = master
            .send_slice(
                &buf1[..],
                Some(&[fd.as_raw_fd(), fd.as_raw_fd(), fd.as_raw_fd()]),
            )
            .unwrap();
        assert_eq!(len, 4);

        let (bytes, _, files) = slave.recv_into_buf(0x4).unwrap();
        assert_eq!(bytes, 4);
        assert!(files.is_some());
    }

    #[test]
    fn send_recv() {
        let path = temp_path();
        let listener = Listener::new(&path, true).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut master = Endpoint::connect(&path).unwrap();
        let sock = listener.accept().unwrap().unwrap();
        let mut slave = Endpoint::from_stream(sock);

        let mut hdr1 = MsgHeader::new(2, RequestCode::GetBlob, 0, mem::size_of::<u64>() as u32);
        hdr1.set_need_reply(true);
        let features1 = 0x1u64;
        master.send_message(&hdr1, &features1, None).unwrap();

        let mut features2 = 0u64;
        let slice = unsafe {
            slice::from_raw_parts_mut(
                (&mut features2 as *mut u64) as *mut u8,
                mem::size_of::<u64>(),
            )
        };
        let (hdr2, bytes, files) = slave.recv_body_into_buf(slice).unwrap();
        assert_eq!(hdr1, hdr2);
        assert_eq!(bytes, 8);
        assert_eq!(features1, features2);
        assert!(files.is_none());

        master.send_header(&hdr1, None).unwrap();
        let (hdr2, files) = slave.recv_header().unwrap();
        assert_eq!(hdr1, hdr2);
        assert!(files.is_none());
    }
}
