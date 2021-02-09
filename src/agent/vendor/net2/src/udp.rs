// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cell::RefCell;
use std::fmt;
use std::io;
use std::net::{ToSocketAddrs, UdpSocket};

use IntoInner;
use socket::Socket;
use sys::c;

/// An "in progress" UDP socket which has not yet been connected.
///
/// Allows configuration of a socket before the socket is connected.
pub struct UdpBuilder {
    socket: RefCell<Option<Socket>>,
}

impl UdpBuilder {
    /// Constructs a new UdpBuilder with the `AF_INET` domain, the `SOCK_DGRAM`
    /// type, and with a protocol argument of 0.
    ///
    /// Note that passing other kinds of flags or arguments can be done through
    /// the `FromRaw{Fd,Socket}` implementation.
    pub fn new_v4() -> io::Result<UdpBuilder> {
        Socket::new(c::AF_INET, c::SOCK_DGRAM).map(::FromInner::from_inner)
    }

    /// Constructs a new UdpBuilder with the `AF_INET6` domain, the `SOCK_DGRAM`
    /// type, and with a protocol argument of 0.
    ///
    /// Note that passing other kinds of flags or arguments can be done through
    /// the `FromRaw{Fd,Socket}` implementation.
    pub fn new_v6() -> io::Result<UdpBuilder> {
        Socket::new(c::AF_INET6, c::SOCK_DGRAM).map(::FromInner::from_inner)
    }

    /// Binds this socket to the specified address.
    ///
    /// This function directly corresponds to the bind(2) function on Windows
    /// and Unix.
    pub fn bind<T>(&self, addr: T) -> io::Result<UdpSocket>
        where T: ToSocketAddrs
    {
        try!(self.with_socket(|sock| {
            let addr = try!(::one_addr(addr));
            sock.bind(&addr)
        }));
        Ok(self.socket.borrow_mut().take().unwrap().into_inner().into_udp_socket())
    }

    fn with_socket<F>(&self, f: F) -> io::Result<()>
        where F: FnOnce(&Socket) -> io::Result<()>
    {
        match *self.socket.borrow() {
            Some(ref s) => f(s),
            None => Err(io::Error::new(io::ErrorKind::Other,
                                       "builder has already finished its socket")),
        }
    }
}

impl fmt::Debug for UdpBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UdpBuilder {{ socket: {:?} }}",
               self.socket.borrow().as_ref().unwrap())
    }
}

impl ::AsInner for UdpBuilder {
    type Inner = RefCell<Option<Socket>>;
    fn as_inner(&self) -> &RefCell<Option<Socket>> { &self.socket }
}

impl ::FromInner for UdpBuilder {
    type Inner = Socket;
    fn from_inner(sock: Socket) -> UdpBuilder {
        UdpBuilder { socket: RefCell::new(Some(sock)) }
    }
}

