//! Unix-specific extensions to the `std::net` types.

use std::io;
use sys::c::{self, c_int};

use {TcpBuilder, UdpBuilder};
use ext::{self, AsSock};

/// Unix-specific extensions for the `TcpBuilder` type in this library.
pub trait UnixTcpBuilderExt {
    /// Set value for the `SO_REUSEPORT` option on this socket.
    ///
    /// This indicates that further calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    fn reuse_port(&self, reuse: bool) -> io::Result<&Self>;

    /// Check the value of the `SO_REUSEPORT` option on this socket.
    fn get_reuse_port(&self) -> io::Result<bool>;
}

impl UnixTcpBuilderExt for TcpBuilder {
    fn reuse_port(&self, reuse: bool) -> io::Result<&Self> {
        ext::set_opt(self.as_sock(), c::SOL_SOCKET, c::SO_REUSEPORT,
                    reuse as c_int).map(|()| self)
    }

    fn get_reuse_port(&self) -> io::Result<bool> {
        ext::get_opt(self.as_sock(), c::SOL_SOCKET, c::SO_REUSEPORT)
            .map(ext::int2bool)
    }
}

/// Unix-specific extensions for the `UdpBuilder` type in this library.
pub trait UnixUdpBuilderExt {
    /// Set value for the `SO_REUSEPORT` option on this socket.
    ///
    /// This indicates that further calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    fn reuse_port(&self, reuse: bool) -> io::Result<&Self>;

    /// Check the value of the `SO_REUSEPORT` option on this socket.
    fn get_reuse_port(&self) -> io::Result<bool>;
}

impl UnixUdpBuilderExt for UdpBuilder {
    fn reuse_port(&self, reuse: bool) -> io::Result<&Self> {
        ext::set_opt(self.as_sock(), c::SOL_SOCKET, c::SO_REUSEPORT,
                    reuse as c_int).map(|()| self)
    }

    fn get_reuse_port(&self) -> io::Result<bool> {
        ext::get_opt(self.as_sock(), c::SOL_SOCKET, c::SO_REUSEPORT)
            .map(ext::int2bool)
    }
}
