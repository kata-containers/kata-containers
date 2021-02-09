//! Primitives for working with UDP
//!
//! The types provided in this module are non-blocking by default and are
//! designed to be portable across all supported Mio platforms. As long as the
//! [portability guidelines] are followed, the behavior should be identical no
//! matter the target platform.
//!
//! [portability guidelines]: ../struct.Poll.html#portability

#![allow(deprecated)]

use {sys, Ready, Poll, PollOpt, Token};
use io::{self, MapNonBlock};
use event::Evented;
use poll::SelectorId;
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr};

/// A User Datagram Protocol socket.
///
/// This is an implementation of a bound UDP socket. This supports both IPv4 and
/// IPv6 addresses, and there is no corresponding notion of a server because UDP
/// is a datagram protocol.
#[derive(Debug)]
pub struct UdpSocket {
    sys: sys::UdpSocket,
    selector_id: SelectorId,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    pub fn bind(addr: &SocketAddr) -> io::Result<UdpSocket> {
        let socket = net::UdpSocket::bind(addr)?;
        UdpSocket::from_socket(socket)
    }

    /// Creates a new mio-wrapped socket from an underlying and bound std
    /// socket.
    ///
    /// This function requires that `socket` has previously been bound to an
    /// address to work correctly, and returns an I/O object which can be used
    /// with mio to send/receive UDP messages.
    ///
    /// This can be used in conjunction with net2's `UdpBuilder` interface to
    /// configure a socket before it's handed off to mio, such as setting
    /// options like `reuse_address` or binding to multiple addresses.
    pub fn from_socket(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        Ok(UdpSocket {
            sys: sys::UdpSocket::new(socket)?,
            selector_id: SelectorId::new(),
        })
    }

    /// Returns the socket address that this socket was created from.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.sys.local_addr()
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UdpSocket` is a reference to the same socket that this
    /// object references. Both handles will read and write the same port, and
    /// options set on one socket will be propagated to the other.
    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.sys.try_clone()
            .map(|s| {
                UdpSocket {
                    sys: s,
                    selector_id: self.selector_id.clone(),
                }
            })
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// Address type can be any implementor of `ToSocketAddrs` trait. See its
    /// documentation for concrete examples.
    pub fn send_to(&self, buf: &[u8], target: &SocketAddr)
                   -> io::Result<Option<usize>> {
        self.sys.send_to(buf, target).map_non_block()
    }

    /// Receives data from the socket and stores data in the supplied buffer `buf`. On success,
    /// returns the number of bytes read and the address from whence the data came.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// The function does not read from `buf`, but is overwriting previous content of `buf`.
    ///
    /// Assuming the function has read `n` bytes, slicing `&buf[..n]` provides
    /// efficient access with iterators and boundary checks.
    pub fn recv_from(&self, buf: &mut [u8])
                     -> io::Result<Option<(usize, SocketAddr)>> {
        self.sys.recv_from(buf).map_non_block()
    }

    /// Sends data on the socket to the address previously bound via connect(). On success,
    /// returns the number of bytes written.
    pub fn send(&self, buf: &[u8])
                   -> io::Result<Option<usize>> {
        self.sys.send(buf).map_non_block()
    }

    /// Receives data from the socket previously bound with connect() and stores data in
    /// the supplied buffer `buf`. On success, returns the number of bytes read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// The function does not read from `buf`, but is overwriting previous content of `buf`.
    ///
    /// Assuming the function has read `n` bytes, slicing `&buf[..n]` provides
    /// efficient access with iterators and boundary checks.
    pub fn recv(&self, buf: &mut [u8])
                     -> io::Result<Option<usize>> {
        self.sys.recv(buf).map_non_block()
    }

    /// Connects the UDP socket setting the default destination for `send()` 
    /// and limiting packets that are read via `recv` from the address specified
    /// in `addr`.
    pub fn connect(&self, addr: SocketAddr)
                 -> io::Result<()> {
        self.sys.connect(addr)
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_broadcast`][link].
    ///
    /// [link]: #method.set_broadcast
    pub fn broadcast(&self) -> io::Result<bool> {
        self.sys.broadcast()
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.sys.set_broadcast(on)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v4`][link].
    ///
    /// [link]: #method.set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.sys.multicast_loop_v4()
    }

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.sys.set_multicast_loop_v4(on)
    }

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_ttl_v4`][link].
    ///
    /// [link]: #method.set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.sys.multicast_ttl_v4()
    }

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.sys.set_multicast_ttl_v4(ttl)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v6`][link].
    ///
    /// [link]: #method.set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.sys.multicast_loop_v6()
    }

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.sys.set_multicast_loop_v6(on)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`][link].
    ///
    /// [link]: #method.set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        self.sys.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.sys.set_ttl(ttl)
    }

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    pub fn join_multicast_v4(&self,
                             multiaddr: &Ipv4Addr,
                             interface: &Ipv4Addr) -> io::Result<()> {
        self.sys.join_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    pub fn join_multicast_v6(&self,
                             multiaddr: &Ipv6Addr,
                             interface: u32) -> io::Result<()> {
        self.sys.join_multicast_v6(multiaddr, interface)
    }

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v4`][link].
    ///
    /// [link]: #method.join_multicast_v4
    pub fn leave_multicast_v4(&self,
                              multiaddr: &Ipv4Addr,
                              interface: &Ipv4Addr) -> io::Result<()> {
        self.sys.leave_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v6`][link].
    ///
    /// [link]: #method.join_multicast_v6
    pub fn leave_multicast_v6(&self,
                              multiaddr: &Ipv6Addr,
                              interface: u32) -> io::Result<()> {
        self.sys.leave_multicast_v6(multiaddr, interface)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.sys.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.selector_id.associate_selector(poll)?;
        self.sys.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.sys.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.sys.deregister(poll)
    }
}

/*
 *
 * ===== UNIX ext =====
 *
 */

#[cfg(all(unix, not(target_os = "fuchsia")))]
use std::os::unix::io::{IntoRawFd, AsRawFd, FromRawFd, RawFd};

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl IntoRawFd for UdpSocket {
    fn into_raw_fd(self) -> RawFd {
        self.sys.into_raw_fd()
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.sys.as_raw_fd()
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl FromRawFd for UdpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> UdpSocket {
        UdpSocket {
            sys: FromRawFd::from_raw_fd(fd),
            selector_id: SelectorId::new(),
        }
    }
}
