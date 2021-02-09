// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(target_os = "linux")]
use std::ffi::{CStr, CString};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{self, Ipv4Addr, Ipv6Addr, Shutdown};
#[cfg(all(unix, feature = "unix"))]
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::time::Duration;

#[cfg(any(unix, target_os = "redox"))]
use libc::MSG_OOB;
#[cfg(windows)]
use winapi::um::winsock2::MSG_OOB;

use crate::sys;
use crate::{Domain, Protocol, SockAddr, Type};

/// Newtype, owned, wrapper around a system socket.
///
/// This type simply wraps an instance of a file descriptor (`c_int`) on Unix
/// and an instance of `SOCKET` on Windows. This is the main type exported by
/// this crate and is intended to mirror the raw semantics of sockets on
/// platforms as closely as possible. Almost all methods correspond to
/// precisely one libc or OS API call which is essentially just a "Rustic
/// translation" of what's below.
///
/// # Examples
///
/// ```no_run
/// use std::net::SocketAddr;
/// use socket2::{Socket, Domain, Type, SockAddr};
///
/// // create a TCP listener bound to two addresses
/// let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
///
/// socket.bind(&"127.0.0.1:12345".parse::<SocketAddr>().unwrap().into()).unwrap();
/// socket.bind(&"127.0.0.1:12346".parse::<SocketAddr>().unwrap().into()).unwrap();
/// socket.listen(128).unwrap();
///
/// let listener = socket.into_tcp_listener();
/// // ...
/// ```
pub struct Socket {
    // The `sys` module most have access to the socket.
    pub(crate) inner: sys::Socket,
}

impl Socket {
    /// Creates a new socket ready to be configured.
    ///
    /// This function corresponds to `socket(2)` and simply creates a new
    /// socket, no other configuration is done and further functions must be
    /// invoked to configure this socket.
    pub fn new(domain: Domain, type_: Type, protocol: Option<Protocol>) -> io::Result<Socket> {
        let protocol = protocol.map(|p| p.0).unwrap_or(0);
        Ok(Socket {
            inner: sys::Socket::new(domain.0, type_.0, protocol)?,
        })
    }

    /// Creates a pair of sockets which are connected to each other.
    ///
    /// This function corresponds to `socketpair(2)`.
    ///
    /// This function is only available on Unix when the `pair` feature is
    /// enabled.
    #[cfg(all(unix, feature = "pair"))]
    pub fn pair(
        domain: Domain,
        type_: Type,
        protocol: Option<Protocol>,
    ) -> io::Result<(Socket, Socket)> {
        let protocol = protocol.map(|p| p.0).unwrap_or(0);
        let sockets = sys::Socket::pair(domain.0, type_.0, protocol)?;
        Ok((Socket { inner: sockets.0 }, Socket { inner: sockets.1 }))
    }

    /// Consumes this `Socket`, converting it to a `TcpStream`.
    pub fn into_tcp_stream(self) -> net::TcpStream {
        self.into()
    }

    /// Consumes this `Socket`, converting it to a `TcpListener`.
    pub fn into_tcp_listener(self) -> net::TcpListener {
        self.into()
    }

    /// Consumes this `Socket`, converting it to a `UdpSocket`.
    pub fn into_udp_socket(self) -> net::UdpSocket {
        self.into()
    }

    /// Consumes this `Socket`, converting it into a `UnixStream`.
    ///
    /// This function is only available on Unix when the `unix` feature is
    /// enabled.
    #[cfg(all(unix, feature = "unix"))]
    pub fn into_unix_stream(self) -> UnixStream {
        self.into()
    }

    /// Consumes this `Socket`, converting it into a `UnixListener`.
    ///
    /// This function is only available on Unix when the `unix` feature is
    /// enabled.
    #[cfg(all(unix, feature = "unix"))]
    pub fn into_unix_listener(self) -> UnixListener {
        self.into()
    }

    /// Consumes this `Socket`, converting it into a `UnixDatagram`.
    ///
    /// This function is only available on Unix when the `unix` feature is
    /// enabled.
    #[cfg(all(unix, feature = "unix"))]
    pub fn into_unix_datagram(self) -> UnixDatagram {
        self.into()
    }

    /// Initiate a connection on this socket to the specified address.
    ///
    /// This function directly corresponds to the connect(2) function on Windows
    /// and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn connect(&self, addr: &SockAddr) -> io::Result<()> {
        self.inner.connect(addr)
    }

    /// Initiate a connection on this socket to the specified address, only
    /// only waiting for a certain period of time for the connection to be
    /// established.
    ///
    /// Unlike many other methods on `Socket`, this does *not* correspond to a
    /// single C function. It sets the socket to nonblocking mode, connects via
    /// connect(2), and then waits for the connection to complete with poll(2)
    /// on Unix and select on Windows. When the connection is complete, the
    /// socket is set back to blocking mode. On Unix, this will loop over
    /// `EINTR` errors.
    ///
    /// # Warnings
    ///
    /// The nonblocking state of the socket is overridden by this function -
    /// it will be returned in blocking mode on success, and in an indeterminate
    /// state on failure.
    ///
    /// If the connection request times out, it may still be processing in the
    /// background - a second call to `connect` or `connect_timeout` may fail.
    pub fn connect_timeout(&self, addr: &SockAddr, timeout: Duration) -> io::Result<()> {
        self.inner.connect_timeout(addr, timeout)
    }

    /// Binds this socket to the specified address.
    ///
    /// This function directly corresponds to the bind(2) function on Windows
    /// and Unix.
    pub fn bind(&self, addr: &SockAddr) -> io::Result<()> {
        self.inner.bind(addr)
    }

    /// Mark a socket as ready to accept incoming connection requests using
    /// accept()
    ///
    /// This function directly corresponds to the listen(2) function on Windows
    /// and Unix.
    ///
    /// An error will be returned if `listen` or `connect` has already been
    /// called on this builder.
    pub fn listen(&self, backlog: i32) -> io::Result<()> {
        self.inner.listen(backlog)
    }

    /// Accept a new incoming connection from this listener.
    ///
    /// This function will block the calling thread until a new connection is
    /// established. When established, the corresponding `Socket` and the
    /// remote peer's address will be returned.
    pub fn accept(&self) -> io::Result<(Socket, SockAddr)> {
        self.inner
            .accept()
            .map(|(socket, addr)| (Socket { inner: socket }, addr))
    }

    /// Returns the socket address of the local half of this TCP connection.
    pub fn local_addr(&self) -> io::Result<SockAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    pub fn peer_addr(&self) -> io::Result<SockAddr> {
        self.inner.peer_addr()
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `TcpStream` is a reference to the same stream that this
    /// object references. Both handles will read and write the same stream of
    /// data, and options set on one stream will be propagated to the other
    /// stream.
    pub fn try_clone(&self) -> io::Result<Socket> {
        self.inner.try_clone().map(|s| Socket { inner: s })
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// On Unix this corresponds to calling fcntl, and on Windows this
    /// corresponds to calling ioctlsocket.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    ///
    /// [`connect`]: #method.connect
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.recv(buf, 0)
    }

    /// Identical to [`recv`] but allows for specification of arbitrary flags to the underlying
    /// `recv` call.
    ///
    /// [`recv`]: #method.recv
    pub fn recv_with_flags(&self, buf: &mut [u8], flags: i32) -> io::Result<usize> {
        self.inner.recv(buf, flags)
    }

    /// Receives out-of-band (OOB) data on the socket from the remote address to
    /// which it is connected by setting the `MSG_OOB` flag for this call.
    ///
    /// For more information, see [`recv`], [`out_of_band_inline`].
    ///
    /// [`recv`]: #method.recv
    /// [`out_of_band_inline`]: #method.out_of_band_inline
    pub fn recv_out_of_band(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.recv(buf, MSG_OOB)
    }

    /// Receives data on the socket from the remote adress to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SockAddr)> {
        self.inner.recv_from(buf, 0)
    }

    /// Identical to [`recv_from`] but allows for specification of arbitrary flags to the underlying
    /// `recvfrom` call.
    ///
    /// [`recv_from`]: #method.recv_from
    pub fn recv_from_with_flags(
        &self,
        buf: &mut [u8],
        flags: i32,
    ) -> io::Result<(usize, SockAddr)> {
        self.inner.recv_from(buf, flags)
    }

    /// Receives data from the socket, without removing it from the queue.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recvfrom` system call.
    ///
    /// On success, returns the number of bytes peeked and the address from
    /// whence the data came.
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SockAddr)> {
        self.inner.peek_from(buf)
    }

    /// Sends data on the socket to a connected peer.
    ///
    /// This is typically used on TCP sockets or datagram sockets which have
    /// been connected.
    ///
    /// On success returns the number of bytes that were sent.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.send(buf, 0)
    }

    /// Identical to [`send`] but allows for specification of arbitrary flags to the underlying
    /// `send` call.
    ///
    /// [`send`]: #method.send
    pub fn send_with_flags(&self, buf: &[u8], flags: i32) -> io::Result<usize> {
        self.inner.send(buf, flags)
    }

    /// Sends out-of-band (OOB) data on the socket to connected peer
    /// by setting the `MSG_OOB` flag for this call.
    ///
    /// For more information, see [`send`], [`out_of_band_inline`].
    ///
    /// [`send`]: #method.send
    /// [`out_of_band_inline`]: #method.out_of_band_inline
    pub fn send_out_of_band(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.send(buf, MSG_OOB)
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// This is typically used on UDP or datagram-oriented sockets. On success
    /// returns the number of bytes that were sent.
    pub fn send_to(&self, buf: &[u8], addr: &SockAddr) -> io::Result<usize> {
        self.inner.send_to(buf, 0, addr)
    }

    /// Identical to [`send_to`] but allows for specification of arbitrary flags to the underlying
    /// `sendto` call.
    ///
    /// [`send_to`]: #method.send_to
    pub fn send_to_with_flags(&self, buf: &[u8], addr: &SockAddr, flags: i32) -> io::Result<usize> {
        self.inner.send_to(buf, flags, addr)
    }

    // ================================================

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`][link].
    ///
    /// [link]: #method.set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `TCP_MAXSEG` option on this socket.
    ///
    /// The `TCP_MAXSEG` option denotes the TCP Maximum Segment
    /// Size and is only available on TCP sockets.
    #[cfg(all(unix, not(target_os = "redox")))]
    pub fn mss(&self) -> io::Result<u32> {
        self.inner.mss()
    }

    /// Sets the value of the `TCP_MAXSEG` option on this socket.
    ///
    /// The `TCP_MAXSEG` option denotes the TCP Maximum Segment
    /// Size and is only available on TCP sockets.
    #[cfg(all(unix, not(target_os = "redox")))]
    pub fn set_mss(&self, mss: u32) -> io::Result<()> {
        self.inner.set_mss(mss)
    }

    /// Gets the value for the `SO_MARK` option on this socket.
    ///
    /// This value gets the socket mark field for each packet sent through
    /// this socket.
    ///
    /// This function is only available on Linux and requires the
    /// `CAP_NET_ADMIN` capability.
    #[cfg(target_os = "linux")]
    pub fn mark(&self) -> io::Result<u32> {
        self.inner.mark()
    }

    /// Sets the value for the `SO_MARK` option on this socket.
    ///
    /// This value sets the socket mark field for each packet sent through
    /// this socket. Changing the mark can be used for mark-based routing
    /// without netfilter or for packet filtering.
    ///
    /// This function is only available on Linux and requires the
    /// `CAP_NET_ADMIN` capability.
    #[cfg(target_os = "linux")]
    pub fn set_mark(&self, mark: u32) -> io::Result<()> {
        self.inner.set_mark(mark)
    }

    /// Gets the value for the `SO_BINDTODEVICE` option on this socket.
    ///
    /// This value gets the socket binded device's interface name.
    ///
    /// This function is only available on Linux.
    #[cfg(target_os = "linux")]
    pub fn device(&self) -> io::Result<Option<CString>> {
        self.inner.device()
    }

    /// Sets the value for the `SO_BINDTODEVICE` option on this socket.
    ///
    /// If a socket is bound to an interface, only packets received from that
    /// particular interface are processed by the socket. Note that this only
    /// works for some socket types, particularly `AF_INET` sockets.
    ///
    /// If `interface` is `None` or an empty string it removes the binding.
    ///
    /// This function is only available on Linux.
    #[cfg(target_os = "linux")]
    pub fn bind_device(&self, interface: Option<&CStr>) -> io::Result<()> {
        self.inner.bind_device(interface)
    }

    /// Gets the value of the `IPV6_UNICAST_HOPS` option for this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    pub fn unicast_hops_v6(&self) -> io::Result<u32> {
        self.inner.unicast_hops_v6()
    }

    /// Sets the value for the `IPV6_UNICAST_HOPS` option on this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    pub fn set_unicast_hops_v6(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_unicast_hops_v6(ttl)
    }

    /// Gets the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see [`set_only_v6`][link].
    ///
    /// [link]: #method.set_only_v6
    pub fn only_v6(&self) -> io::Result<bool> {
        self.inner.only_v6()
    }

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// If this is set to `true` then the socket is restricted to sending and
    /// receiving IPv6 packets only. In this case two IPv4 and IPv6 applications
    /// can bind the same port at the same time.
    ///
    /// If this is set to `false` then the socket can be used to send and
    /// receive packets from an IPv4-mapped IPv6 address.
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.inner.set_only_v6(only_v6)
    }

    /// Returns the read timeout of this socket.
    ///
    /// If the timeout is `None`, then `read` calls will block indefinitely.
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.read_timeout()
    }

    /// Sets the read timeout to the timeout specified.
    ///
    /// If the value specified is `None`, then `read` calls will block
    /// indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(dur)
    }

    /// Returns the write timeout of this socket.
    ///
    /// If the timeout is `None`, then `write` calls will block indefinitely.
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.write_timeout()
    }

    /// Sets the write timeout to the timeout specified.
    ///
    /// If the value specified is `None`, then `write` calls will block
    /// indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(dur)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`][link].
    ///
    /// [link]: #method.set_nodelay
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that
    /// segments are always sent as soon as possible, even if there is only a
    /// small amount of data. When not set, data is buffered until there is a
    /// sufficient amount to send out, thereby avoiding the frequent sending of
    /// small packets.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    pub fn broadcast(&self) -> io::Result<bool> {
        self.inner.broadcast()
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_broadcast`][link].
    ///
    /// [link]: #method.set_broadcast
    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        self.inner.set_broadcast(broadcast)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v4`][link].
    ///
    /// [link]: #method.set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.inner.multicast_loop_v4()
    }

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        self.inner.set_multicast_loop_v4(multicast_loop_v4)
    }

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_ttl_v4`][link].
    ///
    /// [link]: #method.set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.inner.multicast_ttl_v4()
    }

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        self.inner.set_multicast_ttl_v4(multicast_ttl_v4)
    }

    /// Gets the value of the `IPV6_MULTICAST_HOPS` option for this socket
    ///
    /// For more information about this option, see
    /// [`set_multicast_hops_v6`][link].
    ///
    /// [link]: #method.set_multicast_hops_v6
    pub fn multicast_hops_v6(&self) -> io::Result<u32> {
        self.inner.multicast_hops_v6()
    }

    /// Sets the value of the `IPV6_MULTICAST_HOPS` option for this socket
    ///
    /// Indicates the number of "routers" multicast packets will transit for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    pub fn set_multicast_hops_v6(&self, hops: u32) -> io::Result<()> {
        self.inner.set_multicast_hops_v6(hops)
    }

    /// Gets the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_if_v4`][link].
    ///
    /// [link]: #method.set_multicast_if_v4
    ///
    /// Returns the interface to use for routing multicast packets.
    pub fn multicast_if_v4(&self) -> io::Result<Ipv4Addr> {
        self.inner.multicast_if_v4()
    }

    /// Sets the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets.
    pub fn set_multicast_if_v4(&self, interface: &Ipv4Addr) -> io::Result<()> {
        self.inner.set_multicast_if_v4(interface)
    }

    /// Gets the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_if_v6`][link].
    ///
    /// [link]: #method.set_multicast_if_v6
    ///
    /// Returns the interface to use for routing multicast packets.
    pub fn multicast_if_v6(&self) -> io::Result<u32> {
        self.inner.multicast_if_v6()
    }

    /// Sets the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets. Unlike ipv4, this
    /// is generally required in ipv6 contexts where network routing prefixes may
    /// overlap.
    pub fn set_multicast_if_v6(&self, interface: u32) -> io::Result<()> {
        self.inner.set_multicast_if_v6(interface)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v6`][link].
    ///
    /// [link]: #method.set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.inner.multicast_loop_v6()
    }

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        self.inner.set_multicast_loop_v6(multicast_loop_v6)
    }

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.inner.join_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.inner.join_multicast_v6(multiaddr, interface)
    }

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v4`][link].
    ///
    /// [link]: #method.join_multicast_v4
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.inner.leave_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v6`][link].
    ///
    /// [link]: #method.join_multicast_v6
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.inner.leave_multicast_v6(multiaddr, interface)
    }

    /// Reads the linger duration for this socket by getting the SO_LINGER
    /// option
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.inner.linger()
    }

    /// Sets the linger duration of this socket by setting the SO_LINGER option
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_linger(dur)
    }

    /// Check the `SO_REUSEADDR` option on this socket.
    pub fn reuse_address(&self) -> io::Result<bool> {
        self.inner.reuse_address()
    }

    /// Set value for the `SO_REUSEADDR` option on this socket.
    ///
    /// This indicates that futher calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    pub fn set_reuse_address(&self, reuse: bool) -> io::Result<()> {
        self.inner.set_reuse_address(reuse)
    }

    /// Gets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see
    /// [`set_recv_buffer_size`][link].
    ///
    /// [link]: #method.set_recv_buffer_size
    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.inner.recv_buffer_size()
    }

    /// Sets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated
    /// with the socket.
    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.inner.set_recv_buffer_size(size)
    }

    /// Gets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer`][link].
    ///
    /// [link]: #method.set_send_buffer
    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.inner.send_buffer_size()
    }

    /// Sets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with
    /// the socket.
    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.inner.set_send_buffer_size(size)
    }

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the duration of time between them.
    ///
    /// For more information about this option, see [`set_keepalive`][link].
    ///
    /// [link]: #method.set_keepalive
    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.inner.keepalive()
    }

    /// Sets whether keepalive messages are enabled to be sent on this socket.
    ///
    /// On Unix, this option will set the `SO_KEEPALIVE` as well as the
    /// `TCP_KEEPALIVE` or `TCP_KEEPIDLE` option (depending on your platform).
    /// On Windows, this will set the `SIO_KEEPALIVE_VALS` option.
    ///
    /// If `None` is specified then keepalive messages are disabled, otherwise
    /// the duration specified will be the time to remain idle before sending a
    /// TCP keepalive probe.
    ///
    /// Some platforms specify this value in seconds, so sub-second
    /// specifications may be omitted.
    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.inner.set_keepalive(keepalive)
    }

    /// Returns the value of the `SO_OOBINLINE` flag of the underlying socket.
    /// For more information about this option, see [`set_out_of_band_inline`][link].
    ///
    /// [link]: #method.set_out_of_band_inline
    pub fn out_of_band_inline(&self) -> io::Result<bool> {
        self.inner.out_of_band_inline()
    }

    /// Sets the `SO_OOBINLINE` flag of the underlying socket.
    /// as per RFC6093, TCP sockets using the Urgent mechanism
    /// are encouraged to set this flag.
    ///
    /// If this flag is not set, the `MSG_OOB` flag is needed
    /// while `recv`ing to aquire the out-of-band data.
    pub fn set_out_of_band_inline(&self, oob_inline: bool) -> io::Result<()> {
        self.inner.set_out_of_band_inline(oob_inline)
    }

    /// Check the value of the `SO_REUSEPORT` option on this socket.
    ///
    /// This function is only available on Unix when the `reuseport` feature is
    /// enabled.
    #[cfg(all(
        unix,
        not(any(target_os = "solaris", target_os = "illumos")),
        feature = "reuseport"
    ))]
    pub fn reuse_port(&self) -> io::Result<bool> {
        self.inner.reuse_port()
    }

    /// Set value for the `SO_REUSEPORT` option on this socket.
    ///
    /// This indicates that further calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    ///
    /// This function is only available on Unix when the `reuseport` feature is
    /// enabled.
    #[cfg(all(
        unix,
        not(any(target_os = "solaris", target_os = "illumos")),
        feature = "reuseport"
    ))]
    pub fn set_reuse_port(&self, reuse: bool) -> io::Result<()> {
        self.inner.set_reuse_port(reuse)
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a> Read for &'a Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(buf)
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Write for &'a Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

impl fmt::Debug for Socket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<net::TcpStream> for Socket {
    fn from(socket: net::TcpStream) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

impl From<net::TcpListener> for Socket {
    fn from(socket: net::TcpListener) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

impl From<net::UdpSocket> for Socket {
    fn from(socket: net::UdpSocket) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<UnixStream> for Socket {
    fn from(socket: UnixStream) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<UnixListener> for Socket {
    fn from(socket: UnixListener) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<UnixDatagram> for Socket {
    fn from(socket: UnixDatagram) -> Socket {
        Socket {
            inner: socket.into(),
        }
    }
}

impl From<Socket> for net::TcpStream {
    fn from(socket: Socket) -> net::TcpStream {
        socket.inner.into()
    }
}

impl From<Socket> for net::TcpListener {
    fn from(socket: Socket) -> net::TcpListener {
        socket.inner.into()
    }
}

impl From<Socket> for net::UdpSocket {
    fn from(socket: Socket) -> net::UdpSocket {
        socket.inner.into()
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<Socket> for UnixStream {
    fn from(socket: Socket) -> UnixStream {
        socket.inner.into()
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<Socket> for UnixListener {
    fn from(socket: Socket) -> UnixListener {
        socket.inner.into()
    }
}

#[cfg(all(unix, feature = "unix"))]
impl From<Socket> for UnixDatagram {
    fn from(socket: Socket) -> UnixDatagram {
        socket.inner.into()
    }
}

#[cfg(test)]
mod test {
    use std::net::SocketAddr;

    use super::*;

    #[test]
    fn connect_timeout_unrouteable() {
        // this IP is unroutable, so connections should always time out
        let addr = "10.255.255.1:80".parse::<SocketAddr>().unwrap().into();

        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        match socket.connect_timeout(&addr, Duration::from_millis(250)) {
            Ok(_) => panic!("unexpected success"),
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(e) => panic!("unexpected error {}", e),
        }
    }

    #[test]
    fn connect_timeout_unbound() {
        // bind and drop a socket to track down a "probably unassigned" port
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        let addr = "127.0.0.1:0".parse::<SocketAddr>().unwrap().into();
        socket.bind(&addr).unwrap();
        let addr = socket.local_addr().unwrap();
        drop(socket);

        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        match socket.connect_timeout(&addr, Duration::from_millis(250)) {
            Ok(_) => panic!("unexpected success"),
            Err(ref e)
                if e.kind() == io::ErrorKind::ConnectionRefused
                    || e.kind() == io::ErrorKind::TimedOut => {}
            Err(e) => panic!("unexpected error {}", e),
        }
    }

    #[test]
    fn connect_timeout_valid() {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        socket
            .bind(&"127.0.0.1:0".parse::<SocketAddr>().unwrap().into())
            .unwrap();
        socket.listen(128).unwrap();

        let addr = socket.local_addr().unwrap();

        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        socket
            .connect_timeout(&addr, Duration::from_millis(250))
            .unwrap();
    }

    #[test]
    #[cfg(all(unix, feature = "pair", feature = "unix"))]
    fn pair() {
        let (mut a, mut b) = Socket::pair(Domain::unix(), Type::stream(), None).unwrap();
        a.write_all(b"hello world").unwrap();
        let mut buf = [0; 11];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(buf, &b"hello world"[..]);
    }

    #[test]
    #[cfg(all(unix, feature = "unix"))]
    fn unix() {
        use tempdir::TempDir;

        let dir = TempDir::new("unix").unwrap();
        let addr = SockAddr::unix(dir.path().join("sock")).unwrap();

        let listener = Socket::new(Domain::unix(), Type::stream(), None).unwrap();
        listener.bind(&addr).unwrap();
        listener.listen(10).unwrap();

        let mut a = Socket::new(Domain::unix(), Type::stream(), None).unwrap();
        a.connect(&addr).unwrap();

        let mut b = listener.accept().unwrap().0;

        a.write_all(b"hello world").unwrap();
        let mut buf = [0; 11];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(buf, &b"hello world"[..]);
    }

    #[test]
    fn keepalive() {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        socket.set_keepalive(Some(Duration::from_secs(7))).unwrap();
        // socket.keepalive() doesn't work on Windows #24
        #[cfg(unix)]
        assert_eq!(socket.keepalive().unwrap(), Some(Duration::from_secs(7)));
        socket.set_keepalive(None).unwrap();
        #[cfg(unix)]
        assert_eq!(socket.keepalive().unwrap(), None);
    }

    #[test]
    fn nodelay() {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();

        assert!(socket.set_nodelay(true).is_ok());

        let result = socket.nodelay();

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn out_of_band_inline() {
        let socket = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();

        assert_eq!(socket.out_of_band_inline().unwrap(), false);

        socket.set_out_of_band_inline(true).unwrap();
        assert_eq!(socket.out_of_band_inline().unwrap(), true);
    }

    #[test]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn out_of_band_send_recv() {
        let s1 = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        s1.bind(&"127.0.0.1:0".parse::<SocketAddr>().unwrap().into())
            .unwrap();
        let s1_addr = s1.local_addr().unwrap();
        s1.listen(1).unwrap();

        let s2 = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        s2.connect(&s1_addr).unwrap();

        let (s3, _) = s1.accept().unwrap();

        let mut buf = [0; 10];
        // send some plain inband data
        s2.send(&mut buf).unwrap();
        // send a single out of band byte
        assert_eq!(s2.send_out_of_band(&mut [b"!"[0]]).unwrap(), 1);
        // recv the OOB data first
        assert_eq!(s3.recv_out_of_band(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], b"!"[0]);
        assert_eq!(s3.recv(&mut buf).unwrap(), 10);
    }

    #[test]
    fn tcp() {
        let s1 = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        s1.bind(&"127.0.0.1:0".parse::<SocketAddr>().unwrap().into())
            .unwrap();
        let s1_addr = s1.local_addr().unwrap();
        s1.listen(1).unwrap();

        let s2 = Socket::new(Domain::ipv4(), Type::stream(), None).unwrap();
        s2.connect(&s1_addr).unwrap();

        let (s3, _) = s1.accept().unwrap();

        let mut buf = [0; 11];
        assert_eq!(s2.send(&mut buf).unwrap(), 11);
        assert_eq!(s3.recv(&mut buf).unwrap(), 11);
    }
}
