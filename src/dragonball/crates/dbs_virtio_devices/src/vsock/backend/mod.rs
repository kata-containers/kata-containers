// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// This module implements backends for vsock - the host side vsock endpoint,
/// which can translate vsock stream into host's protocol, eg. AF_UNIX, AF_INET
/// or even the protocol created by us.
use std::any::Any;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

mod hybrid_stream;
mod inner;
mod tcp;
mod unix_stream;

pub use self::hybrid_stream::HybridStream;
pub use self::inner::{VsockInnerBackend, VsockInnerConnector, VsockInnerStream};
pub use self::tcp::VsockTcpBackend;
pub use self::unix_stream::VsockUnixStreamBackend;

/// The type of vsock backend.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum VsockBackendType {
    /// Unix stream
    UnixStream,
    /// Tcp socket
    Tcp,
    /// Inner backend
    Inner,
    /// Fd passed hybrid stream backend
    HybridStream,
    /// For test purpose
    #[cfg(test)]
    Test,
}

/// The generic abstract of Vsock Backend, looks like socket's API.
pub trait VsockBackend: AsRawFd + Send {
    /// Accept a host-initiated connection.
    fn accept(&mut self) -> std::io::Result<Box<dyn VsockStream>>;
    /// Connect by a guest-initiated connection.
    fn connect(&self, dst_port: u32) -> std::io::Result<Box<dyn VsockStream>>;
    /// The type of backend.
    fn r#type(&self) -> VsockBackendType;
    /// Used to downcast to the specific type.
    fn as_any(&self) -> &dyn Any;
}

/// The generic abstract of Vsock Stream.
pub trait VsockStream: Read + Write + AsRawFd + Send {
    /// The type of backend which created the stream.
    fn backend_type(&self) -> VsockBackendType;
    /// Moves VsockStream into or out of nonblocking mode
    fn set_nonblocking(&mut self, _nonblocking: bool) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
    }
    /// Set the read timeout to the time duration specified.
    fn set_read_timeout(&mut self, _dur: Option<Duration>) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }
    /// Set the write timeout to the time duration specified.
    fn set_write_timeout(&mut self, _dur: Option<Duration>) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }
    /// Receive the port and fd from the peer.
    fn recv_data_fd(
        &self,
        _bytes: &mut [u8],
        _fds: &mut [RawFd],
    ) -> std::io::Result<(usize, usize)> {
        Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }
    /// Used to downcast to the specific type.
    fn as_any(&self) -> &dyn Any;
}
