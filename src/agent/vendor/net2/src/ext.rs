// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(bad_style, dead_code)]

use std::io;
use std::mem;
use std::net::{TcpStream, TcpListener, UdpSocket, Ipv4Addr, Ipv6Addr};
use std::net::ToSocketAddrs;

use {TcpBuilder, UdpBuilder, FromInner};
use sys;
use sys::c;
use socket;

cfg_if! {
    if #[cfg(any(target_os = "dragonfly",
                 target_os = "freebsd",
                 target_os = "haiku",
                 target_os = "ios",
                 target_os = "macos",
                 target_os = "netbsd",
                 target_os = "openbsd",
                 target_os = "solaris",
                 target_os = "illumos",
                 target_env = "uclibc"))] {
        use libc::IPV6_JOIN_GROUP as IPV6_ADD_MEMBERSHIP;
        use libc::IPV6_LEAVE_GROUP as IPV6_DROP_MEMBERSHIP;
    } else {
        // ...
    }
}

use std::time::Duration;

#[cfg(any(unix, target_os = "redox", target_os = "wasi"))] use libc::*;
#[cfg(any(unix, target_os = "redox"))] use std::os::unix::prelude::*;
#[cfg(target_os = "wasi")] use std::os::wasi::prelude::*;
#[cfg(target_os = "redox")] pub type Socket = usize;
#[cfg(unix)] pub type Socket = c_int;
#[cfg(target_os = "wasi")] pub type Socket = std::os::wasi::io::RawFd;
#[cfg(windows)] pub type Socket = SOCKET;
#[cfg(windows)] use std::os::windows::prelude::*;
#[cfg(any(windows, target_os = "wasi"))] use sys::c::*;

#[cfg(windows)] const SIO_KEEPALIVE_VALS: DWORD = 0x98000004;
#[cfg(windows)]
#[repr(C)]
struct tcp_keepalive {
    onoff: c_ulong,
    keepalivetime: c_ulong,
    keepaliveinterval: c_ulong,
}

#[cfg(any(unix, target_os = "redox", target_os = "wasi"))] fn v(opt: c_int) -> c_int { opt }
#[cfg(windows)] fn v(opt: IPPROTO) -> c_int { opt as c_int }

#[cfg(target_os = "wasi")]
pub fn set_opt<T: Copy>(_sock: Socket, _opt: c_int, _val: c_int,
                       _payload: T) -> io::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "wasi"))]
pub fn set_opt<T: Copy>(sock: Socket, opt: c_int, val: c_int,
                       payload: T) -> io::Result<()> {
    unsafe {
        let payload = &payload as *const T as *const c_void;
        #[cfg(target_os = "redox")]
        let sock = sock as c_int;
        try!(::cvt(setsockopt(sock, opt, val, payload as *const _,
                              mem::size_of::<T>() as socklen_t)));
    }
    Ok(())
}

#[cfg(target_os = "wasi")]
pub fn get_opt<T: Copy>(_sock: Socket, _opt: c_int, _val: c_int) -> io::Result<T> {
    unimplemented!()
}
#[cfg(not(target_os = "wasi"))]
pub fn get_opt<T: Copy>(sock: Socket, opt: c_int, val: c_int) -> io::Result<T> {
    unsafe {
        let mut slot: T = mem::zeroed();
        let mut len = mem::size_of::<T>() as socklen_t;
        #[cfg(target_os = "redox")]
        let sock = sock as c_int;
        try!(::cvt(getsockopt(sock, opt, val,
                              &mut slot as *mut _ as *mut _,
                              &mut len)));
        assert_eq!(len as usize, mem::size_of::<T>());
        Ok(slot)
    }
}

/// Extension methods for the standard [`TcpStream` type][link] in `std::net`.
///
/// [link]: https://doc.rust-lang.org/std/net/struct.TcpStream.html
pub trait TcpStreamExt {
    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that
    /// segments are always sent as soon as possible, even if there is only a
    /// small amount of data. When not set, data is buffered until there is a
    /// sufficient amount to send out, thereby avoiding the frequent sending of
    /// small packets.
    fn set_nodelay(&self, nodelay: bool) -> io::Result<()>;

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`][link].
    ///
    /// [link]: #tymethod.set_nodelay
    fn nodelay(&self) -> io::Result<bool>;

    /// Sets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated with the socket.
    fn set_recv_buffer_size(&self, size: usize) -> io::Result<()>;

    /// Gets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_recv_buffer_size`][link].
    ///
    /// [link]: #tymethod.set_recv_buffer_size
    fn recv_buffer_size(&self) -> io::Result<usize>;

    /// Sets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with the socket.
    fn set_send_buffer_size(&self, size: usize) -> io::Result<()>;

    /// Gets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer`][link].
    ///
    /// [link]: #tymethod.set_send_buffer
    fn send_buffer_size(&self) -> io::Result<usize>;

    /// Sets whether keepalive messages are enabled to be sent on this socket.
    ///
    /// On Unix, this option will set the `SO_KEEPALIVE` as well as the
    /// `TCP_KEEPALIVE` or `TCP_KEEPIDLE` option (depending on your platform).
    /// On Windows, this will set the `SIO_KEEPALIVE_VALS` option.
    ///
    /// If `None` is specified then keepalive messages are disabled, otherwise
    /// the number of milliseconds specified will be the time to remain idle
    /// before sending a TCP keepalive probe.
    ///
    /// Some platforms specify this value in seconds, so sub-second millisecond
    /// specifications may be omitted.
    fn set_keepalive_ms(&self, keepalive: Option<u32>) -> io::Result<()>;

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the amount of milliseconds between them.
    ///
    /// For more information about this option, see [`set_keepalive_ms`][link].
    ///
    /// [link]: #tymethod.set_keepalive_ms
    fn keepalive_ms(&self) -> io::Result<Option<u32>>;

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
    fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()>;

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the duration of time between them.
    ///
    /// For more information about this option, see [`set_keepalive`][link].
    ///
    /// [link]: #tymethod.set_keepalive
    fn keepalive(&self) -> io::Result<Option<Duration>>;

    /// Sets the `SO_RCVTIMEO` option for this socket.
    ///
    /// This option specifies the timeout, in milliseconds, of how long calls to
    /// this socket's `read` function will wait before returning a timeout. A
    /// value of `None` means that no read timeout should be specified and
    /// otherwise `Some` indicates the number of milliseconds for the timeout.
    fn set_read_timeout_ms(&self, val: Option<u32>) -> io::Result<()>;

    /// Sets the `SO_RCVTIMEO` option for this socket.
    ///
    /// This option specifies the timeout of how long calls to this socket's
    /// `read` function will wait before returning a timeout. A value of `None`
    /// means that no read timeout should be specified and otherwise `Some`
    /// indicates the number of duration of the timeout.
    fn set_read_timeout(&self, val: Option<Duration>) -> io::Result<()>;

    /// Gets the value of the `SO_RCVTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_read_timeout_ms`][link].
    ///
    /// [link]: #tymethod.set_read_timeout_ms
    fn read_timeout_ms(&self) -> io::Result<Option<u32>>;

    /// Gets the value of the `SO_RCVTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_read_timeout`][link].
    ///
    /// [link]: #tymethod.set_read_timeout
    fn read_timeout(&self) -> io::Result<Option<Duration>>;

    /// Sets the `SO_SNDTIMEO` option for this socket.
    ///
    /// This option specifies the timeout, in milliseconds, of how long calls to
    /// this socket's `write` function will wait before returning a timeout. A
    /// value of `None` means that no read timeout should be specified and
    /// otherwise `Some` indicates the number of milliseconds for the timeout.
    fn set_write_timeout_ms(&self, val: Option<u32>) -> io::Result<()>;

    /// Sets the `SO_SNDTIMEO` option for this socket.
    ///
    /// This option specifies the timeout of how long calls to this socket's
    /// `write` function will wait before returning a timeout. A value of `None`
    /// means that no read timeout should be specified and otherwise `Some`
    /// indicates the duration of the timeout.
    fn set_write_timeout(&self, val: Option<Duration>) -> io::Result<()>;

    /// Gets the value of the `SO_SNDTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_write_timeout_ms`][link].
    ///
    /// [link]: #tymethod.set_write_timeout_ms
    fn write_timeout_ms(&self) -> io::Result<Option<u32>>;

    /// Gets the value of the `SO_SNDTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_write_timeout`][link].
    ///
    /// [link]: #tymethod.set_write_timeout
    fn write_timeout(&self) -> io::Result<Option<Duration>>;

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    fn set_ttl(&self, ttl: u32) -> io::Result<()>;

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`][link].
    ///
    /// [link]: #tymethod.set_ttl
    fn ttl(&self) -> io::Result<u32>;

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// If this is set to `true` then the socket is restricted to sending and
    /// receiving IPv6 packets only. In this case two IPv4 and IPv6 applications
    /// can bind the same port at the same time.
    ///
    /// If this is set to `false` then the socket can be used to send and
    /// receive packets from an IPv4-mapped IPv6 address.
    fn set_only_v6(&self, only_v6: bool) -> io::Result<()>;

    /// Gets the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see [`set_only_v6`][link].
    ///
    /// [link]: #tymethod.set_only_v6
    fn only_v6(&self) -> io::Result<bool>;

    /// Executes a `connect` operation on this socket, establishing a connection
    /// to the host specified by `addr`.
    ///
    /// Note that this normally does not need to be called on a `TcpStream`,
    /// it's typically automatically done as part of a normal
    /// `TcpStream::connect` function call or `TcpBuilder::connect` method call.
    ///
    /// This should only be necessary if an unconnected socket was extracted
    /// from a `TcpBuilder` and then needs to be connected.
    fn connect<T: ToSocketAddrs>(&self, addr: T) -> io::Result<()>;

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    fn take_error(&self) -> io::Result<Option<io::Error>>;

    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// On Unix this corresponds to calling fcntl, and on Windows this
    /// corresponds to calling ioctlsocket.
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;

    /// Sets the linger duration of this socket by setting the SO_LINGER option
    fn set_linger(&self, dur: Option<Duration>) -> io::Result<()>;

    /// reads the linger duration for this socket by getting the SO_LINGER option
    fn linger(&self) -> io::Result<Option<Duration>>;
}

/// Extension methods for the standard [`TcpListener` type][link] in `std::net`.
///
/// [link]: https://doc.rust-lang.org/std/net/struct.TcpListener.html
pub trait TcpListenerExt {
    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This is the same as [`TcpStreamExt::set_ttl`][other].
    ///
    /// [other]: trait.TcpStreamExt.html#tymethod.set_ttl
    fn set_ttl(&self, ttl: u32) -> io::Result<()>;

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_ttl`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_ttl
    fn ttl(&self) -> io::Result<u32>;

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_only_v6`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_only_v6
    fn set_only_v6(&self, only_v6: bool) -> io::Result<()>;

    /// Gets the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_only_v6`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_only_v6
    fn only_v6(&self) -> io::Result<bool>;

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    fn take_error(&self) -> io::Result<Option<io::Error>>;

    /// Moves this TCP listener into or out of nonblocking mode.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_nonblocking`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_nonblocking
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;

    /// Sets the linger duration of this socket by setting the SO_LINGER option
    fn set_linger(&self, dur: Option<Duration>) -> io::Result<()>;

    /// reads the linger duration for this socket by getting the SO_LINGER option
    fn linger(&self) -> io::Result<Option<Duration>>;
}

/// Extension methods for the standard [`UdpSocket` type][link] in `std::net`.
///
/// [link]: https://doc.rust-lang.org/std/net/struct.UdpSocket.html
pub trait UdpSocketExt {
    /// Sets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated with the socket.
    fn set_recv_buffer_size(&self, size: usize) -> io::Result<()>;

    /// Gets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_recv_buffer_size`][link].
    ///
    /// [link]: #tymethod.set_recv_buffer_size
    fn recv_buffer_size(&self) -> io::Result<usize>;

    /// Sets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with the socket.
    fn set_send_buffer_size(&self, size: usize) -> io::Result<()>;

    /// Gets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer`][link].
    ///
    /// [link]: #tymethod.set_send_buffer
    fn send_buffer_size(&self) -> io::Result<usize>;

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    fn set_broadcast(&self, broadcast: bool) -> io::Result<()>;

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_broadcast`][link].
    ///
    /// [link]: #tymethod.set_broadcast
    fn broadcast(&self) -> io::Result<bool>;

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()>;

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v4`][link].
    ///
    /// [link]: #tymethod.set_multicast_loop_v4
    fn multicast_loop_v4(&self) -> io::Result<bool>;

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()>;

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_ttl_v4`][link].
    ///
    /// [link]: #tymethod.set_multicast_ttl_v4
    fn multicast_ttl_v4(&self) -> io::Result<u32>;

    /// Sets the value of the `IPV6_MULTICAST_HOPS` option for this socket
    fn set_multicast_hops_v6(&self, hops: u32) -> io::Result<()>;

    /// Gets the value of the `IPV6_MULTICAST_HOPS` option for this socket
    fn multicast_hops_v6(&self) -> io::Result<u32>;

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()>;

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v6`][link].
    ///
    /// [link]: #tymethod.set_multicast_loop_v6
    fn multicast_loop_v6(&self) -> io::Result<bool>;

    /// Sets the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets.
    fn set_multicast_if_v4(&self, interface: &Ipv4Addr) -> io::Result<()>;

    /// Gets the value of the `IP_MULTICAST_IF` option for this socket.
    ///
    /// Returns the interface to use for routing multicast packets.
    fn multicast_if_v4(&self) -> io::Result<Ipv4Addr>;


    /// Sets the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// Specifies the interface to use for routing multicast packets.
    fn set_multicast_if_v6(&self, interface: u32) -> io::Result<()>;

    /// Gets the value of the `IPV6_MULTICAST_IF` option for this socket.
    ///
    /// Returns the interface to use for routing multicast packets.
    fn multicast_if_v6(&self) -> io::Result<u32>;

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This is the same as [`TcpStreamExt::set_ttl`][other].
    ///
    /// [other]: trait.TcpStreamExt.html#tymethod.set_ttl
    fn set_ttl(&self, ttl: u32) -> io::Result<()>;

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_ttl`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_ttl
    fn ttl(&self) -> io::Result<u32>;

    /// Sets the value for the `IPV6_UNICAST_HOPS` option on this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    fn set_unicast_hops_v6(&self, ttl: u32) -> io::Result<()>;

    /// Gets the value of the `IPV6_UNICAST_HOPS` option for this socket.
    ///
    /// Specifies the hop limit for ipv6 unicast packets
    fn unicast_hops_v6(&self) -> io::Result<u32>;

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_only_v6`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_only_v6
    fn set_only_v6(&self, only_v6: bool) -> io::Result<()>;

    /// Gets the value of the `IPV6_V6ONLY` option for this socket.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_only_v6`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_only_v6
    fn only_v6(&self) -> io::Result<bool>;

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr)
                         -> io::Result<()>;

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32)
                         -> io::Result<()>;

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v4`][link].
    ///
    /// [link]: #tymethod.join_multicast_v4
    fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr)
                          -> io::Result<()>;

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v6`][link].
    ///
    /// [link]: #tymethod.join_multicast_v6
    fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32)
                          -> io::Result<()>;

    /// Sets the `SO_RCVTIMEO` option for this socket.
    ///
    /// This option specifies the timeout, in milliseconds, of how long calls to
    /// this socket's `read` function will wait before returning a timeout. A
    /// value of `None` means that no read timeout should be specified and
    /// otherwise `Some` indicates the number of milliseconds for the timeout.
    fn set_read_timeout_ms(&self, val: Option<u32>) -> io::Result<()>;

    /// Sets the `SO_RCVTIMEO` option for this socket.
    ///
    /// This option specifies the timeout of how long calls to this socket's
    /// `read` function will wait before returning a timeout. A value of `None`
    /// means that no read timeout should be specified and otherwise `Some`
    /// indicates the number of duration of the timeout.
    fn set_read_timeout(&self, val: Option<Duration>) -> io::Result<()>;

    /// Gets the value of the `SO_RCVTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_read_timeout_ms`][link].
    ///
    /// [link]: #tymethod.set_read_timeout_ms
    fn read_timeout_ms(&self) -> io::Result<Option<u32>>;

    /// Gets the value of the `SO_RCVTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_read_timeout`][link].
    ///
    /// [link]: #tymethod.set_read_timeout
    fn read_timeout(&self) -> io::Result<Option<Duration>>;

    /// Sets the `SO_SNDTIMEO` option for this socket.
    ///
    /// This option specifies the timeout, in milliseconds, of how long calls to
    /// this socket's `write` function will wait before returning a timeout. A
    /// value of `None` means that no read timeout should be specified and
    /// otherwise `Some` indicates the number of milliseconds for the timeout.
    fn set_write_timeout_ms(&self, val: Option<u32>) -> io::Result<()>;

    /// Sets the `SO_SNDTIMEO` option for this socket.
    ///
    /// This option specifies the timeout of how long calls to this socket's
    /// `write` function will wait before returning a timeout. A value of `None`
    /// means that no read timeout should be specified and otherwise `Some`
    /// indicates the duration of the timeout.
    fn set_write_timeout(&self, val: Option<Duration>) -> io::Result<()>;

    /// Gets the value of the `SO_SNDTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_write_timeout_ms`][link].
    ///
    /// [link]: #tymethod.set_write_timeout_ms
    fn write_timeout_ms(&self) -> io::Result<Option<u32>>;

    /// Gets the value of the `SO_SNDTIMEO` option for this socket.
    ///
    /// For more information about this option, see [`set_write_timeout`][link].
    ///
    /// [link]: #tymethod.set_write_timeout
    fn write_timeout(&self) -> io::Result<Option<Duration>>;

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    fn take_error(&self) -> io::Result<Option<io::Error>>;

    /// Connects this UDP socket to a remote address, allowing the `send` and
    /// `recv` syscalls to be used to send data and also applies filters to only
    /// receive data from the specified address.
    fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()>;

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// The `connect` method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    fn send(&self, buf: &[u8]) -> io::Result<usize>;

    /// Receives data on the socket from the remote address to which it is
    /// connected.
    ///
    /// The `connect` method will connect this socket to a remote address. This
    /// method will fail if the socket is not connected.
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize>;

    /// Moves this UDP socket into or out of nonblocking mode.
    ///
    /// For more information about this option, see
    /// [`TcpStreamExt::set_nonblocking`][link].
    ///
    /// [link]: trait.TcpStreamExt.html#tymethod.set_nonblocking
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
}

#[doc(hidden)]
pub trait AsSock {
    fn as_sock(&self) -> Socket;
}

#[cfg(any(unix, target_os = "redox", target_os = "wasi"))]
impl<T: AsRawFd> AsSock for T {
    fn as_sock(&self) -> Socket { self.as_raw_fd() }
}
#[cfg(windows)]
impl<T: AsRawSocket> AsSock for T {
    fn as_sock(&self) -> Socket { self.as_raw_socket() as Socket }
}

cfg_if! {
    if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        use libc::TCP_KEEPALIVE as KEEPALIVE_OPTION;
    } else if #[cfg(any(target_os = "haiku", target_os = "netbsd", target_os = "openbsd"))] {
        use libc::SO_KEEPALIVE as KEEPALIVE_OPTION;
    } else if #[cfg(unix)] {
        use libc::TCP_KEEPIDLE as KEEPALIVE_OPTION;
    } else if #[cfg(target_os = "redox")] {
        use libc::TCP_KEEPIDLE as KEEPALIVE_OPTION;
    } else {
        // ...
    }
}

impl TcpStreamExt for TcpStream {

    fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        // TODO: casting usize to a c_int should be a checked cast
        set_opt(self.as_sock(), SOL_SOCKET, SO_RCVBUF, size as c_int)
    }

    fn recv_buffer_size(&self) -> io::Result<usize> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_RCVBUF).map(int2usize)
    }

    fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_SNDBUF, size as c_int)
    }

    fn send_buffer_size(&self) -> io::Result<usize> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_SNDBUF).map(int2usize)
    }

    fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        set_opt(self.as_sock(), v(IPPROTO_TCP), TCP_NODELAY,
               nodelay as c_int)
    }
    fn nodelay(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), v(IPPROTO_TCP), TCP_NODELAY)
            .map(int2bool)
    }

    fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.set_keepalive_ms(keepalive.map(dur2ms))
    }

    fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.keepalive_ms().map(|o| o.map(ms2dur))
    }

    #[cfg(target_os = "redox")]
    fn set_keepalive_ms(&self, keepalive: Option<u32>) -> io::Result<()> {
        try!(set_opt(self.as_sock(), SOL_SOCKET, SO_KEEPALIVE,
                    keepalive.is_some() as c_int));
        if let Some(dur) = keepalive {
            try!(set_opt(self.as_sock(), v(IPPROTO_TCP), KEEPALIVE_OPTION,
                        (dur / 1000) as c_int));
        }
        Ok(())
    }

    #[cfg(target_os = "redox")]
    fn keepalive_ms(&self) -> io::Result<Option<u32>> {
        let keepalive = try!(get_opt::<c_int>(self.as_sock(), SOL_SOCKET,
                                             SO_KEEPALIVE));
        if keepalive == 0 {
            return Ok(None)
        }
        let secs = try!(get_opt::<c_int>(self.as_sock(), v(IPPROTO_TCP),
                                        KEEPALIVE_OPTION));
        Ok(Some((secs as u32) * 1000))
    }

    #[cfg(unix)]
    fn set_keepalive_ms(&self, keepalive: Option<u32>) -> io::Result<()> {
        try!(set_opt(self.as_sock(), SOL_SOCKET, SO_KEEPALIVE,
                    keepalive.is_some() as c_int));
        if let Some(dur) = keepalive {
            try!(set_opt(self.as_sock(), v(IPPROTO_TCP), KEEPALIVE_OPTION,
                        (dur / 1000) as c_int));
        }
        Ok(())
    }

    #[cfg(unix)]
    fn keepalive_ms(&self) -> io::Result<Option<u32>> {
        let keepalive = try!(get_opt::<c_int>(self.as_sock(), SOL_SOCKET,
                                             SO_KEEPALIVE));
        if keepalive == 0 {
            return Ok(None)
        }
        let secs = try!(get_opt::<c_int>(self.as_sock(), v(IPPROTO_TCP),
                                        KEEPALIVE_OPTION));
        Ok(Some((secs as u32) * 1000))
    }

     #[cfg(target_os = "wasi")]
    fn set_keepalive_ms(&self, _keepalive: Option<u32>) -> io::Result<()> {
        unimplemented!()
    }

    #[cfg(target_os = "wasi")]
    fn keepalive_ms(&self) -> io::Result<Option<u32>> {
        unimplemented!()
    }

    #[cfg(windows)]
    fn set_keepalive_ms(&self, keepalive: Option<u32>) -> io::Result<()> {
        let ms = keepalive.unwrap_or(INFINITE);
        let ka = tcp_keepalive {
            onoff: keepalive.is_some() as c_ulong,
            keepalivetime: ms as c_ulong,
            keepaliveinterval: ms as c_ulong,
        };
        unsafe {
            ::cvt_win(WSAIoctl(self.as_sock(),
                               SIO_KEEPALIVE_VALS,
                               &ka as *const _ as *mut _,
                               mem::size_of_val(&ka) as DWORD,
                               0 as *mut _,
                               0,
                               0 as *mut _,
                               0 as *mut _,
                               None)).map(|_| ())
        }
    }

    #[cfg(windows)]
    fn keepalive_ms(&self) -> io::Result<Option<u32>> {
        let mut ka = tcp_keepalive {
            onoff: 0,
            keepalivetime: 0,
            keepaliveinterval: 0,
        };
        unsafe {
            try!(::cvt_win(WSAIoctl(self.as_sock(),
                                    SIO_KEEPALIVE_VALS,
                                    0 as *mut _,
                                    0,
                                    &mut ka as *mut _ as *mut _,
                                    mem::size_of_val(&ka) as DWORD,
                                    0 as *mut _,
                                    0 as *mut _,
                                    None)));
        }
        Ok({
            if ka.onoff == 0 {
                None
            } else {
                timeout2ms(ka.keepaliveinterval as DWORD)
            }
        })
    }

    fn set_read_timeout_ms(&self, dur: Option<u32>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_RCVTIMEO,
               ms2timeout(dur))
    }

    fn read_timeout_ms(&self) -> io::Result<Option<u32>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_RCVTIMEO)
            .map(timeout2ms)
    }

    fn set_write_timeout_ms(&self, dur: Option<u32>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_SNDTIMEO,
               ms2timeout(dur))
    }

    fn write_timeout_ms(&self) -> io::Result<Option<u32>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_SNDTIMEO)
            .map(timeout2ms)
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout_ms(dur.map(dur2ms))
    }

    fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout_ms().map(|o| o.map(ms2dur))
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout_ms(dur.map(dur2ms))
    }

    fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout_ms().map(|o| o.map(ms2dur))
    }

    fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_TTL, ttl as c_int)
    }

    fn ttl(&self) -> io::Result<u32> {
        get_opt::<c_int>(self.as_sock(), IPPROTO_IP, IP_TTL)
            .map(|b| b as u32)
    }

    fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY, only_v6 as c_int)
    }

    fn only_v6(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY).map(int2bool)
    }

    fn connect<T: ToSocketAddrs>(&self, addr: T) -> io::Result<()> {
        do_connect(self.as_sock(), addr)
    }

    fn take_error(&self) -> io::Result<Option<io::Error>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_ERROR).map(int2err)
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        set_nonblocking(self.as_sock(), nonblocking)
    }

    fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_LINGER, dur2linger(dur))
    }

    fn linger(&self) -> io::Result<Option<Duration>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_LINGER).map(linger2dur)
    }
}

#[cfg(any(target_os = "redox", unix, target_os = "wasi"))]
fn ms2timeout(dur: Option<u32>) -> timeval {
    // TODO: be more rigorous
    match dur {
        Some(d) => timeval {
            tv_sec: (d / 1000) as time_t,
            tv_usec: (d % 1000) as suseconds_t,
        },
        None => timeval { tv_sec: 0, tv_usec: 0 },
    }
}

#[cfg(any(target_os = "redox", unix, target_os = "wasi"))]
fn timeout2ms(dur: timeval) -> Option<u32> {
    if dur.tv_sec == 0 && dur.tv_usec == 0 {
        None
    } else {
        Some(dur.tv_sec as u32 * 1000 + dur.tv_usec as u32 / 1000)
    }
}

#[cfg(windows)]
fn ms2timeout(dur: Option<u32>) -> DWORD {
    dur.unwrap_or(0)
}

#[cfg(windows)]
fn timeout2ms(dur: DWORD) -> Option<u32> {
    if dur == 0 {
        None
    } else {
        Some(dur)
    }
}

fn linger2dur(linger_opt: linger) -> Option<Duration> {
    if linger_opt.l_onoff == 0 {
        None
    }
    else {
        Some(Duration::from_secs(linger_opt.l_linger as u64))
    }
}

#[cfg(windows)]
fn dur2linger(dur: Option<Duration>) -> linger {
    match dur {
        Some(d) => {
            linger {
                l_onoff: 1,
                l_linger: d.as_secs() as u16,
            }
        },
        None => linger { l_onoff: 0, l_linger: 0 },
    }
}

#[cfg(any(target_os = "redox", unix, target_os = "wasi"))]
fn dur2linger(dur: Option<Duration>) -> linger {
    match dur {
        Some(d) => {
            linger {
                l_onoff: 1,
                l_linger: d.as_secs() as c_int,
            }
        },
        None => linger { l_onoff: 0, l_linger: 0 },
    }
}

fn ms2dur(ms: u32) -> Duration {
    Duration::new((ms as u64) / 1000, (ms as u32) % 1000 * 1_000_000)
}

fn dur2ms(dur: Duration) -> u32 {
    (dur.as_secs() as u32 * 1000) + (dur.subsec_nanos() / 1_000_000)
}

pub fn int2bool(n: c_int) -> bool {
    if n == 0 {false} else {true}
}

pub fn int2usize(n: c_int) -> usize {
    // TODO: casting c_int to a usize should be a checked cast
    n as usize
}

pub fn int2err(n: c_int) -> Option<io::Error> {
    if n == 0 {
        None
    } else {
        Some(io::Error::from_raw_os_error(n as i32))
    }
}

impl UdpSocketExt for UdpSocket {

    fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_RCVBUF, size as c_int)
    }

    fn recv_buffer_size(&self) -> io::Result<usize> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_RCVBUF).map(int2usize)
    }

    fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_SNDBUF, size as c_int)
    }

    fn send_buffer_size(&self) -> io::Result<usize> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_SNDBUF).map(int2usize)
    }

    fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_BROADCAST,
               broadcast as c_int)
    }
    fn broadcast(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_BROADCAST)
            .map(int2bool)
    }
    fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_MULTICAST_LOOP,
               multicast_loop_v4 as c_int)
    }
    fn multicast_loop_v4(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), IPPROTO_IP, IP_MULTICAST_LOOP)
            .map(int2bool)
    }

    fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_MULTICAST_TTL,
               multicast_ttl_v4 as c_int)
    }
    
    fn multicast_ttl_v4(&self) -> io::Result<u32> {
        get_opt::<c_int>(self.as_sock(), IPPROTO_IP, IP_MULTICAST_TTL)
            .map(|b| b as u32)
    }

    fn set_multicast_hops_v6(&self, _hops: u32) -> io::Result<()> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_HOPS,
               _hops as c_int)
    }

    fn multicast_hops_v6(&self) -> io::Result<u32> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        get_opt::<c_int>(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_HOPS)
            .map(|b| b as u32)
    }

    fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_LOOP,
               multicast_loop_v6 as c_int)
    }
    fn multicast_loop_v6(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_LOOP)
            .map(int2bool)
    }

    fn set_multicast_if_v4(&self, _interface: &Ipv4Addr) -> io::Result<()> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        set_opt(self.as_sock(), IPPROTO_IP, IP_MULTICAST_IF, ip2in_addr(_interface))
    }

    fn multicast_if_v4(&self) -> io::Result<Ipv4Addr> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        get_opt(self.as_sock(), IPPROTO_IP, IP_MULTICAST_IF).map(in_addr2ip)
    }

    fn set_multicast_if_v6(&self, _interface: u32) -> io::Result<()> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_IF, to_ipv6mr_interface(_interface))
    }

    fn multicast_if_v6(&self) -> io::Result<u32> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        get_opt::<c_int>(self.as_sock(), v(IPPROTO_IPV6), IPV6_MULTICAST_IF).map(|b| b as u32)
    }

    fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_TTL, ttl as c_int)
    }

    fn ttl(&self) -> io::Result<u32> {
        get_opt::<c_int>(self.as_sock(), IPPROTO_IP, IP_TTL)
            .map(|b| b as u32)
    }

    fn set_unicast_hops_v6(&self, _ttl: u32) -> io::Result<()> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_UNICAST_HOPS, _ttl as c_int)
    }

    fn unicast_hops_v6(&self) -> io::Result<u32> {
        #[cfg(target_os = "redox")]
        return Err(io::Error::new(io::ErrorKind::Other, "Not implemented yet"));
        #[cfg(not(target_os = "redox"))]
        get_opt::<c_int>(self.as_sock(), IPPROTO_IP, IPV6_UNICAST_HOPS)
            .map(|b| b as u32)
    }

    fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY, only_v6 as c_int)
    }

    fn only_v6(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY).map(int2bool)
    }

    fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr)
                         -> io::Result<()> {
        let mreq = ip_mreq {
            imr_multiaddr: ip2in_addr(multiaddr),
            imr_interface: ip2in_addr(interface),
        };
        set_opt(self.as_sock(), IPPROTO_IP, IP_ADD_MEMBERSHIP, mreq)
    }

    fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32)
                         -> io::Result<()> {
        let mreq = ipv6_mreq {
            ipv6mr_multiaddr: ip2in6_addr(multiaddr),
            ipv6mr_interface: to_ipv6mr_interface(interface),
        };
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_ADD_MEMBERSHIP,
               mreq)
    }

    fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr)
                          -> io::Result<()> {
        let mreq = ip_mreq {
            imr_multiaddr: ip2in_addr(multiaddr),
            imr_interface: ip2in_addr(interface),
        };
        set_opt(self.as_sock(), IPPROTO_IP, IP_DROP_MEMBERSHIP, mreq)
    }

    fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32)
                          -> io::Result<()> {
        let mreq = ipv6_mreq {
            ipv6mr_multiaddr: ip2in6_addr(multiaddr),
            ipv6mr_interface: to_ipv6mr_interface(interface),
        };
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_DROP_MEMBERSHIP,
               mreq)
    }

    fn set_read_timeout_ms(&self, dur: Option<u32>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_RCVTIMEO,
               ms2timeout(dur))
    }

    fn read_timeout_ms(&self) -> io::Result<Option<u32>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_RCVTIMEO)
            .map(timeout2ms)
    }

    fn set_write_timeout_ms(&self, dur: Option<u32>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_SNDTIMEO,
               ms2timeout(dur))
    }

    fn write_timeout_ms(&self) -> io::Result<Option<u32>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_SNDTIMEO)
            .map(timeout2ms)
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout_ms(dur.map(dur2ms))
    }

    fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout_ms().map(|o| o.map(ms2dur))
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout_ms(dur.map(dur2ms))
    }

    fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout_ms().map(|o| o.map(ms2dur))
    }

    fn take_error(&self) -> io::Result<Option<io::Error>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_ERROR).map(int2err)
    }

    fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        do_connect(self.as_sock(), addr)
    }

    #[cfg(target_os = "redox")]
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            ::cvt(write(self.as_sock() as c_int, buf.as_ptr() as *const _, buf.len())).map(|n| n as usize)
        }
    }

    #[cfg(unix)]
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            ::cvt(send(self.as_sock() as c_int, buf.as_ptr() as *const _, buf.len(), 0)).map(|n| n as usize)
        }
    }

     #[cfg(target_os = "wasi")]
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let _so_datalen: *mut sys::c::size_t = &mut 0;
        unsafe {
            let _errno = libc::__wasi_sock_send(
                self.as_sock() as libc::__wasi_fd_t,
                buf.as_ptr() as *const _,
                buf.len(),
                0,
                _so_datalen,
            );
            // TODO: handle errno
            Ok((*_so_datalen) as usize)
        }
    }

    #[cfg(windows)]
    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let len = ::std::cmp::min(buf.len(), c_int::max_value() as usize);
        let buf = &buf[..len];
        unsafe {
            ::cvt(send(self.as_sock(), buf.as_ptr() as *const _, len as c_int, 0))
                .map(|n| n as usize)
        }
    }

    #[cfg(target_os = "redox")]
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            ::cvt(read(self.as_sock() as c_int, buf.as_mut_ptr() as *mut _, buf.len()))
                .map(|n| n as usize)
        }
    }

    #[cfg(unix)]
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            ::cvt(recv(self.as_sock(), buf.as_mut_ptr() as *mut _, buf.len(), 0))
                .map(|n| n as usize)
        }
    }

    #[cfg(target_os = "wasi")]
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let _ro_datalen: *mut sys::c::size_t = &mut 0;
        let _ro_flags: *mut sys::c::__wasi_roflags_t = &mut 0;
        unsafe {
            let _errno = __wasi_sock_recv(
                self.as_sock(),
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                0,
                _ro_datalen,
                _ro_flags,
            );
            // TODO: handle errno
            Ok((*_ro_datalen) as usize)
        }
    }

    #[cfg(windows)]
    fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let len = ::std::cmp::min(buf.len(), c_int::max_value() as usize);
        let buf = &mut buf[..len];
        unsafe {
            ::cvt(recv(self.as_sock(), buf.as_mut_ptr() as *mut _, buf.len() as c_int, 0))
                .map(|n| n as usize)
        }
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        set_nonblocking(self.as_sock(), nonblocking)
    }
}

fn do_connect<A: ToSocketAddrs>(sock: Socket, addr: A) -> io::Result<()> {
    let err = io::Error::new(io::ErrorKind::Other,
                             "no socket addresses resolved");
    let addrs = try!(addr.to_socket_addrs());
    let sys = sys::Socket::from_inner(sock);
    let sock = socket::Socket::from_inner(sys);
    let ret = addrs.fold(Err(err), |prev, addr| {
        prev.or_else(|_| sock.connect(&addr))
    });
    mem::forget(sock);
    return ret
}

#[cfg(target_os = "redox")]
fn set_nonblocking(sock: Socket, nonblocking: bool) -> io::Result<()> {
    let mut flags = ::cvt(unsafe {
        fcntl(sock as c_int, F_GETFL)
    })?;
    if nonblocking {
        flags |= O_NONBLOCK;
    } else {
        flags &= !O_NONBLOCK;
    }
    ::cvt(unsafe {
        fcntl(sock as c_int, F_SETFL, flags)
    }).and(Ok(()))
}

#[cfg(unix)]
fn set_nonblocking(sock: Socket, nonblocking: bool) -> io::Result<()> {
    let mut nonblocking = nonblocking as c_ulong;
    ::cvt(unsafe {
        ioctl(sock, FIONBIO, &mut nonblocking)
    }).map(|_| ())
}

#[cfg(target_os = "wasi")]
fn set_nonblocking(_sock: Socket, _nonblocking: bool) -> io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn set_nonblocking(sock: Socket, nonblocking: bool) -> io::Result<()> {
    let mut nonblocking = nonblocking as c_ulong;
    ::cvt(unsafe {
        ioctlsocket(sock, FIONBIO as c_int, &mut nonblocking)
    }).map(|_| ())
}

#[cfg(target_os = "redox")]
fn ip2in_addr(ip: &Ipv4Addr) -> in_addr {
    let oct = ip.octets();
    in_addr {
        s_addr: ::hton(((oct[0] as u32) << 24) |
                       ((oct[1] as u32) << 16) |
                       ((oct[2] as u32) <<  8) |
                       ((oct[3] as u32) <<  0)),
    }
}

#[cfg(any(unix, target_os = "wasi"))]
fn ip2in_addr(ip: &Ipv4Addr) -> in_addr {
    let oct = ip.octets();
    in_addr {
        s_addr: ::hton(((oct[0] as u32) << 24) |
                       ((oct[1] as u32) << 16) |
                       ((oct[2] as u32) <<  8) |
                       ((oct[3] as u32) <<  0)),
    }
}

#[cfg(windows)]
fn ip2in_addr(ip: &Ipv4Addr) -> in_addr {
    let oct = ip.octets();
    unsafe {
        let mut S_un: in_addr_S_un = mem::zeroed();
        *S_un.S_addr_mut() = ::hton(((oct[0] as u32) << 24) |
                                ((oct[1] as u32) << 16) |
                                ((oct[2] as u32) <<  8) |
                                ((oct[3] as u32) <<  0));
        in_addr {
            S_un: S_un,
        }
    }
}

fn in_addr2ip(ip: &in_addr) -> Ipv4Addr {
    let h_addr = c::in_addr_to_u32(ip);
    
    let a: u8 = (h_addr >> 24) as u8;
    let b: u8 = (h_addr >> 16) as u8;
    let c: u8 = (h_addr >> 8) as u8;
    let d: u8 = (h_addr >> 0) as u8;

    Ipv4Addr::new(a,b,c,d)
}

#[cfg(target_os = "android")]
fn to_ipv6mr_interface(value: u32) -> c_int {
    value as c_int
}

#[cfg(not(target_os = "android"))]
fn to_ipv6mr_interface(value: u32) -> c_uint {
    value as c_uint
}

fn ip2in6_addr(ip: &Ipv6Addr) -> in6_addr {
    let mut ret: in6_addr = unsafe { mem::zeroed() };
    let seg = ip.segments();
    let bytes = [
        (seg[0] >> 8) as u8,
        (seg[0] >> 0) as u8,
        (seg[1] >> 8) as u8,
        (seg[1] >> 0) as u8,
        (seg[2] >> 8) as u8,
        (seg[2] >> 0) as u8,
        (seg[3] >> 8) as u8,
        (seg[3] >> 0) as u8,
        (seg[4] >> 8) as u8,
        (seg[4] >> 0) as u8,
        (seg[5] >> 8) as u8,
        (seg[5] >> 0) as u8,
        (seg[6] >> 8) as u8,
        (seg[6] >> 0) as u8,
        (seg[7] >> 8) as u8,
        (seg[7] >> 0) as u8,
    ];
    #[cfg(windows)] unsafe { *ret.u.Byte_mut() = bytes; }
    #[cfg(not(windows))]   { ret.s6_addr = bytes; }

    return ret
}

impl TcpListenerExt for TcpListener {
    fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_TTL, ttl as c_int)
    }

    fn ttl(&self) -> io::Result<u32> {
        get_opt::<c_int>(self.as_sock(), IPPROTO_IP, IP_TTL)
            .map(|b| b as u32)
    }

    fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY, only_v6 as c_int)
    }

    fn only_v6(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY).map(int2bool)
    }

    fn take_error(&self) -> io::Result<Option<io::Error>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_ERROR).map(int2err)
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        set_nonblocking(self.as_sock(), nonblocking)
    }

    fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_LINGER, dur2linger(dur))
    }

    fn linger(&self) -> io::Result<Option<Duration>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_LINGER).map(linger2dur)
    }
}

impl TcpBuilder {
    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This is the same as [`TcpStreamExt::set_ttl`][other].
    ///
    /// [other]: trait.TcpStreamExt.html#tymethod.set_ttl
    pub fn ttl(&self, ttl: u32) -> io::Result<&Self> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_TTL, ttl as c_int)
            .map(|()| self)
    }

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// This is the same as [`TcpStreamExt::set_only_v6`][other].
    ///
    /// [other]: trait.TcpStreamExt.html#tymethod.set_only_v6
    pub fn only_v6(&self, only_v6: bool) -> io::Result<&Self> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY, only_v6 as c_int)
            .map(|()| self)
    }

    /// Set value for the `SO_REUSEADDR` option on this socket.
    ///
    /// This indicates that further calls to `bind` may allow reuse of local
    /// addresses. For IPv4 sockets this means that a socket may bind even when
    /// there's a socket already listening on this port.
    pub fn reuse_address(&self, reuse: bool) -> io::Result<&Self> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_REUSEADDR,
               reuse as c_int).map(|()| self)
    }

    /// Check the `SO_REUSEADDR` option on this socket.
    pub fn get_reuse_address(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_REUSEADDR).map(int2bool)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_ERROR).map(int2err)
    }

    /// Sets the linger option for this socket
    fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_LINGER, dur2linger(dur))
    }

    /// Gets the linger option for this socket
    fn linger(&self) -> io::Result<Option<Duration>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_LINGER).map(linger2dur)
    }
}

impl UdpBuilder {
    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This is the same as [`TcpStreamExt::set_ttl`][other].
    ///
    /// [other]: trait.TcpStreamExt.html#tymethod.set_ttl
    pub fn ttl(&self, ttl: u32) -> io::Result<&Self> {
        set_opt(self.as_sock(), IPPROTO_IP, IP_TTL, ttl as c_int)
            .map(|()| self)
    }

    /// Sets the value for the `IPV6_V6ONLY` option on this socket.
    ///
    /// This is the same as [`TcpStream::only_v6`][other].
    ///
    /// [other]: struct.TcpBuilder.html#method.only_v6
    pub fn only_v6(&self, only_v6: bool) -> io::Result<&Self> {
        set_opt(self.as_sock(), v(IPPROTO_IPV6), IPV6_V6ONLY, only_v6 as c_int)
            .map(|()| self)
    }

    /// Set value for the `SO_REUSEADDR` option on this socket.
    ///
    /// This is the same as [`TcpBuilder::reuse_address`][other].
    ///
    /// [other]: struct.TcpBuilder.html#method.reuse_address
    pub fn reuse_address(&self, reuse: bool) -> io::Result<&Self> {
        set_opt(self.as_sock(), SOL_SOCKET, SO_REUSEADDR,
               reuse as c_int).map(|()| self)
    }

    /// Check the `SO_REUSEADDR` option on this socket.
    pub fn get_reuse_address(&self) -> io::Result<bool> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_REUSEADDR).map(int2bool)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        get_opt(self.as_sock(), SOL_SOCKET, SO_ERROR).map(int2err)
    }
}
