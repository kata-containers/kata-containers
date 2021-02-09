use std::os::wasi::io::{FromRawFd, AsRawFd};

use {TcpBuilder, UdpBuilder, FromInner, AsInner};
use socket::Socket;
use sys::{self, c::__wasi_fd_t};

impl FromRawFd for TcpBuilder {
    unsafe fn from_raw_fd(fd: __wasi_fd_t) -> TcpBuilder {
        let sock = sys::Socket::from_inner(fd);
        TcpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawFd for TcpBuilder {
    fn as_raw_fd(&self) -> __wasi_fd_t {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as __wasi_fd_t
    }
}

impl FromRawFd for UdpBuilder {
    unsafe fn from_raw_fd(fd: __wasi_fd_t) -> UdpBuilder {
        let sock = sys::Socket::from_inner(fd);
        UdpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawFd for UdpBuilder {
    fn as_raw_fd(&self) -> __wasi_fd_t {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as __wasi_fd_t
    }
}
