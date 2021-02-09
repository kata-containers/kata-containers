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
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, TcpListener, TcpStream};
use std::fmt;

use IntoInner;
use socket::Socket;
use sys::c;

/// An "in progress" TCP socket which has not yet been connected or listened.
///
/// Allows configuration of a socket before one of these operations is executed.
pub struct TcpBuilder {
    socket: RefCell<Option<Socket>>,
}

impl TcpBuilder {
    /// Constructs a new TcpBuilder with the `AF_INET` domain, the `SOCK_STREAM`
    /// type, and with a protocol argument of 0.
    ///
    /// Note that passing other kinds of flags or arguments can be done through
    /// the `FromRaw{Fd,Socket}` implementation.
    pub fn new_v4() -> io::Result<TcpBuilder> {
        Socket::new(c::AF_INET, c::SOCK_STREAM).map(::FromInner::from_inner)
    }

    /// Constructs a new TcpBuilder with the `AF_INET6` domain, the `SOCK_STREAM`
    /// type, and with a protocol argument of 0.
    ///
    /// Note that passing other kinds of flags or arguments can be done through
    /// the `FromRaw{Fd,Socket}` implementation.
    pub fn new_v6() -> io::Result<TcpBuilder> {
        Socket::new(c::AF_INET6, c::SOCK_STREAM).map(::FromInner::from_inner)
    }

    /// Binds this socket to the specified address.
    ///
    /// This function directly corresponds to the bind(2) function on Windows
    /// and Unix.
    pub fn bind<T>(&self, addr: T) -> io::Result<&TcpBuilder>
        where T: ToSocketAddrs
    {
        self.with_socket(|sock| {
            let addr = try!(::one_addr(addr));
            sock.bind(&addr)
        }).map(|()| self)
    }

    /// Mark a socket as ready to accept incoming connection requests using
    /// accept()
    ///
    /// This function directly corresponds to the listen(2) function on Windows
    /// and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn listen(&self, backlog: i32) -> io::Result<TcpListener> {
        self.with_socket(|sock| {
            sock.listen(backlog)
        }).and_then(|()| {
            self.to_tcp_listener()
        })
    }

    /// Initiate a connection on this socket to the specified address.
    ///
    /// This function directly corresponds to the connect(2) function on Windows
    /// and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn connect<T>(&self, addr: T) -> io::Result<TcpStream>
        where T: ToSocketAddrs
    {
        self.with_socket(|sock| {
            let err = io::Error::new(io::ErrorKind::Other,
                                     "no socket addresses resolved");
            try!(addr.to_socket_addrs()).fold(Err(err), |prev, addr| {
                prev.or_else(|_| sock.connect(&addr))
            })
        }).and_then(|()| {
            self.to_tcp_stream()
        })
    }

    /// Converts this builder into a `TcpStream`
    ///
    /// This function will consume the internal socket and return it re-wrapped
    /// as a `TcpStream`. An error will be returned if the internal socket has
    /// already been consumed from a successful call to `connect`, `listen`,
    /// etc.
    pub fn to_tcp_stream(&self) -> io::Result<TcpStream> {
        self.socket.borrow_mut().take().map(|s| s.into_inner().into_tcp_stream())
            .ok_or(io::Error::new(io::ErrorKind::Other,
                                  "socket has already been consumed"))
    }

    /// Converts this builder into a `TcpListener`
    ///
    /// This function will consume the internal socket and return it re-wrapped
    /// as a `TcpListener`. An error will be returned if the internal socket has
    /// already been consumed from a successful call to `connect`, `listen`,
    /// etc.
    pub fn to_tcp_listener(&self) -> io::Result<TcpListener> {
        self.socket.borrow_mut().take()
            .map(|s| s.into_inner().into_tcp_listener())
            .ok_or(io::Error::new(io::ErrorKind::Other,
                                  "socket has already been consumed"))
    }

    /// Returns the address of the local half of this TCP socket.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        match *self.socket.borrow() {
            Some(ref s) => s.getsockname(),
            None => Err(io::Error::new(io::ErrorKind::Other,
                                       "builder has already finished its socket")),
        }
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

impl fmt::Debug for TcpBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TcpBuilder {{ socket: {:?} }}",
               self.socket.borrow().as_ref().unwrap())
    }
}

impl ::AsInner for TcpBuilder {
    type Inner = RefCell<Option<Socket>>;
    fn as_inner(&self) -> &RefCell<Option<Socket>> { &self.socket }
}

impl ::FromInner for TcpBuilder {
    type Inner = Socket;
    fn from_inner(sock: Socket) -> TcpBuilder {
        TcpBuilder { socket: RefCell::new(Some(sock)) }
    }
}
