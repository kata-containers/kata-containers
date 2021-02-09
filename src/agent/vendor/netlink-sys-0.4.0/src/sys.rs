//! Netlink socket related functions
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{Error, Result};
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use super::Protocol;

#[derive(Clone, Debug)]
pub struct Socket(RawFd);

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl FromRawFd for Socket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Socket(fd)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

#[derive(Copy, Clone)]
pub struct SocketAddr(libc::sockaddr_nl);

impl Hash for SocketAddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.nl_family.hash(state);
        self.0.nl_pid.hash(state);
        self.0.nl_groups.hash(state);
    }
}

impl PartialEq for SocketAddr {
    fn eq(&self, other: &SocketAddr) -> bool {
        self.0.nl_family == other.0.nl_family
            && self.0.nl_pid == other.0.nl_pid
            && self.0.nl_groups == other.0.nl_groups
    }
}

impl fmt::Debug for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SocketAddr(nl_family={}, nl_pid={}, nl_groups={})",
            self.0.nl_family, self.0.nl_pid, self.0.nl_groups
        )
    }
}

impl Eq for SocketAddr {}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "address family: {}, pid: {}, multicast groups: {})",
            self.0.nl_family, self.0.nl_pid, self.0.nl_groups
        )
    }
}

impl SocketAddr {
    pub fn new(port_number: u32, multicast_groups: u32) -> Self {
        let mut addr: libc::sockaddr_nl = unsafe { mem::zeroed() };
        addr.nl_family = libc::PF_NETLINK as libc::sa_family_t;
        addr.nl_pid = port_number;
        addr.nl_groups = multicast_groups;
        SocketAddr(addr)
    }

    pub fn port_number(&self) -> u32 {
        self.0.nl_pid
    }

    pub fn multicast_groups(&self) -> u32 {
        self.0.nl_groups
    }

    fn as_raw(&self) -> (*const libc::sockaddr, libc::socklen_t) {
        let addr_ptr = &self.0 as *const libc::sockaddr_nl as *const libc::sockaddr;
        //             \                                 / \                      /
        //              +---------------+---------------+   +----------+---------+
        //                               |                             |
        //                               v                             |
        //             create a raw pointer to the sockaddr_nl         |
        //                                                             v
        //                                                cast *sockaddr_nl -> *sockaddr
        //
        // This kind of things seems to be pretty usual when using C APIs from Rust. It could be
        // written in a shorter way thank to type inference:
        //
        //      let addr_ptr: *const libc:sockaddr = &self.0 as *const _ as *const _;
        //
        // But since this is my first time dealing with this kind of things I chose the most
        // explicit form.

        let addr_len = mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        (addr_ptr, addr_len)
    }

    fn as_raw_mut(&mut self) -> (*mut libc::sockaddr, libc::socklen_t) {
        let addr_ptr = &mut self.0 as *mut libc::sockaddr_nl as *mut libc::sockaddr;
        let addr_len = mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        (addr_ptr, addr_len)
    }
}

impl Socket {
    pub fn new(protocol: Protocol) -> Result<Self> {
        let res =
            unsafe { libc::socket(libc::PF_NETLINK, libc::SOCK_DGRAM, protocol as libc::c_int) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(Socket(res))
    }

    pub fn bind(&mut self, addr: &SocketAddr) -> Result<()> {
        let (addr_ptr, addr_len) = addr.as_raw();
        let res = unsafe { libc::bind(self.0, addr_ptr, addr_len) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn bind_auto(&mut self) -> Result<SocketAddr> {
        let mut addr = SocketAddr::new(0, 0);
        self.bind(&addr)?;
        self.get_address(&mut addr)?;
        Ok(addr)
    }

    pub fn get_address(&self, addr: &mut SocketAddr) -> Result<()> {
        let (addr_ptr, mut addr_len) = addr.as_raw_mut();
        let addr_len_copy = addr_len;
        let addr_len_ptr = &mut addr_len as *mut libc::socklen_t;
        let res = unsafe { libc::getsockname(self.0, addr_ptr, addr_len_ptr) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        assert_eq!(addr_len, addr_len_copy);
        Ok(())
    }

    pub fn set_non_blocking(&self, non_blocking: bool) -> Result<()> {
        let mut non_blocking = non_blocking as libc::c_int;
        let res = unsafe { libc::ioctl(self.0, libc::FIONBIO, &mut non_blocking) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn connect(&self, remote_addr: &SocketAddr) -> Result<()> {
        // Event though for SOCK_DGRAM sockets there's no IO, since our socket is non-blocking,
        // connect() might return EINPROGRESS. In theory, the right way to treat EINPROGRESS would
        // be to ignore the error, and let the user poll the socket to check when it becomes
        // writable, indicating that the connection succeeded. The code already exists in mio for
        // TcpStream:
        //
        // > pub fn connect(stream: net::TcpStream, addr: &SocketAddr) -> io::Result<TcpStream> {
        // >     set_non_block(stream.as_raw_fd())?;
        // >     match stream.connect(addr) {
        // >         Ok(..) => {}
        // >         Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
        // >         Err(e) => return Err(e),
        // >     }
        // >     Ok(TcpStream {  inner: stream })
        // > }
        //
        // The polling to wait for the connection is available in the tokio-tcp crate. See:
        // https://github.com/tokio-rs/tokio/blob/363b207f2b6c25857c70d76b303356db87212f59/tokio-tcp/src/stream.rs#L706
        //
        // In practice, since the connection does not require any IO for SOCK_DGRAM sockets, it
        // almost never returns EINPROGRESS and so for now, we just return whatever libc::connect
        // returns. If it returns EINPROGRESS, the caller will have to handle the error themself
        //
        // Refs:
        //
        // - https://stackoverflow.com/a/14046386/1836144
        // - https://lists.isc.org/pipermail/bind-users/2009-August/077527.html
        let (addr, addr_len) = remote_addr.as_raw();
        let res = unsafe { libc::connect(self.0, addr, addr_len) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    // Most of the comments in this method come from a discussion on rust users forum.
    // [thread]: https://users.rust-lang.org/t/help-understanding-libc-call/17308/9
    //
    // WARNING: with datagram oriented protocols, `recv` and
    // `recvfrom` receive normally only ONE datagram, but it seems not
    // to be verified for Netlink sockets: multiple message can be
    // received in a single call.
    pub fn recv_from(&self, buf: &mut [u8], flags: libc::c_int) -> Result<(usize, SocketAddr)> {
        // Create an empty storage for the address. Note that Rust standard library create a
        // sockaddr_storage so that it works for any address family, but here, we already know that
        // we'll have a Netlink address, so we can create the appropriate storage.
        let mut addr = unsafe { mem::zeroed::<libc::sockaddr_nl>() };

        // recvfrom takes a *sockaddr as parameter so that it can accept any kind of address
        // storage, so we need to create such a pointer for the sockaddr_nl we just initialized.
        //
        //                     Create a raw pointer to        Cast our raw pointer to a
        //                     our storage. We cannot         generic pointer to *sockaddr
        //                     pass it to recvfrom yet.       that recvfrom can use
        //                                 ^                              ^
        //                                 |                              |
        //                  +--------------+---------------+    +---------+--------+
        //                 /                                \  /                    \
        let addr_ptr = &mut addr as *mut libc::sockaddr_nl as *mut libc::sockaddr;

        // Why do we need to pass the address length? We're passing a generic *sockaddr to
        // recvfrom. Somehow recvfrom needs to make sure that the address of the received packet
        // would fit into the actual type that is behind *sockaddr: it could be a sockaddr_nl but
        // also a sockaddr_in, a sockaddr_in6, or even the generic sockaddr_storage that can store
        // any address.
        let mut addrlen = mem::size_of_val(&addr);
        // recvfrom does not take the address length by value (see [thread]), so we need to create
        // a pointer to it.
        let addrlen_ptr = &mut addrlen as *mut usize as *mut libc::socklen_t;

        //                      Cast the *mut u8 into *mut void.
        //               This is equivalent to casting a *char into *void
        //                                 See [thread]
        //                                       ^
        //           Create a *mut u8            |
        //                   ^                   |
        //                   |                   |
        //             +-----+-----+    +--------+-------+
        //            /             \  /                  \
        let buf_ptr = buf.as_mut_ptr() as *mut libc::c_void;
        let buf_len = buf.len() as libc::size_t;

        let res = unsafe { libc::recvfrom(self.0, buf_ptr, buf_len, flags, addr_ptr, addrlen_ptr) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok((res as usize, SocketAddr(addr)))
    }

    pub fn recv(&self, buf: &mut [u8], flags: libc::c_int) -> Result<usize> {
        let buf_ptr = buf.as_mut_ptr() as *mut libc::c_void;
        let buf_len = buf.len() as libc::size_t;

        let res = unsafe { libc::recv(self.0, buf_ptr, buf_len, flags) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(res as usize)
    }

    /// Receive a full message.
    /// Unlike recv_from, which truncates messages that exceed the length of the buffer passed as argument,
    /// this method always reads a whole message, no matter its size.
    pub fn recv_from_full(&self) -> Result<(Vec<u8>, SocketAddr)> {
        // Peek
        let mut buf = Vec::<u8>::new();
        let (rlen, _) = self.recv_from(&mut buf, libc::MSG_PEEK | libc::MSG_TRUNC)?;

        // Receive
        let mut buf = vec![0; rlen as usize];
        let (_, addr) = self.recv_from(&mut buf, 0)?;

        Ok((buf, addr))
    }

    pub fn send_to(&self, buf: &[u8], addr: &SocketAddr, flags: libc::c_int) -> Result<usize> {
        let (addr_ptr, addr_len) = addr.as_raw();
        let buf_ptr = buf.as_ptr() as *const libc::c_void;
        let buf_len = buf.len() as libc::size_t;

        let res = unsafe { libc::sendto(self.0, buf_ptr, buf_len, flags, addr_ptr, addr_len) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(res as usize)
    }

    pub fn send(&self, buf: &[u8], flags: libc::c_int) -> Result<usize> {
        let buf_ptr = buf.as_ptr() as *const libc::c_void;
        let buf_len = buf.len() as libc::size_t;

        let res = unsafe { libc::send(self.0, buf_ptr, buf_len, flags) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(res as usize)
    }

    pub fn set_pktinfo(&mut self, value: bool) -> Result<()> {
        let value: libc::c_int = if value { 1 } else { 0 };
        setsockopt(self.0, libc::SOL_NETLINK, libc::NETLINK_PKTINFO, value)
    }

    pub fn get_pktinfo(&self) -> Result<bool> {
        let res = getsockopt::<libc::c_int>(self.0, libc::SOL_NETLINK, libc::NETLINK_PKTINFO)?;
        Ok(res == 1)
    }

    pub fn add_membership(&mut self, group: u32) -> Result<()> {
        setsockopt(
            self.0,
            libc::SOL_NETLINK,
            libc::NETLINK_ADD_MEMBERSHIP,
            group,
        )
    }

    pub fn drop_membership(&mut self, group: u32) -> Result<()> {
        setsockopt(
            self.0,
            libc::SOL_NETLINK,
            libc::NETLINK_DROP_MEMBERSHIP,
            group,
        )
    }

    // pub fn list_membership(&self) -> Vec<u32> {
    //     unimplemented!();
    //     // getsockopt won't be enough here, because we may need to perform 2 calls, and because the
    //     // length of the list returned by libc::getsockopt is returned by mutating the length
    //     // argument, which our implementation of getsockopt forbids.
    // }

    /// `NETLINK_BROADCAST_ERROR` (since Linux 2.6.30). When not set, `netlink_broadcast()` only
    /// reports `ESRCH` errors and silently ignore `NOBUFS` errors.
    pub fn set_broadcast_error(&mut self, value: bool) -> Result<()> {
        let value: libc::c_int = if value { 1 } else { 0 };
        setsockopt(
            self.0,
            libc::SOL_NETLINK,
            libc::NETLINK_BROADCAST_ERROR,
            value,
        )
    }

    pub fn get_broadcast_error(&self) -> Result<bool> {
        let res =
            getsockopt::<libc::c_int>(self.0, libc::SOL_NETLINK, libc::NETLINK_BROADCAST_ERROR)?;
        Ok(res == 1)
    }

    /// `NETLINK_NO_ENOBUFS` (since Linux 2.6.30). This flag can be used by unicast and broadcast
    /// listeners to avoid receiving `ENOBUFS` errors.
    pub fn set_no_enobufs(&mut self, value: bool) -> Result<()> {
        let value: libc::c_int = if value { 1 } else { 0 };
        setsockopt(self.0, libc::SOL_NETLINK, libc::NETLINK_NO_ENOBUFS, value)
    }

    pub fn get_no_enobufs(&self) -> Result<bool> {
        let res = getsockopt::<libc::c_int>(self.0, libc::SOL_NETLINK, libc::NETLINK_NO_ENOBUFS)?;
        Ok(res == 1)
    }

    /// `NETLINK_LISTEN_ALL_NSID` (since Linux 4.2). When set, this socket will receive netlink
    /// notifications from  all  network  namespaces that have an nsid assigned into the network
    /// namespace where the socket has been opened. The nsid is sent to user space via an ancillary
    /// data.
    pub fn set_listen_all_namespaces(&mut self, value: bool) -> Result<()> {
        let value: libc::c_int = if value { 1 } else { 0 };
        setsockopt(
            self.0,
            libc::SOL_NETLINK,
            libc::NETLINK_LISTEN_ALL_NSID,
            value,
        )
    }

    pub fn get_listen_all_namespaces(&self) -> Result<bool> {
        let res =
            getsockopt::<libc::c_int>(self.0, libc::SOL_NETLINK, libc::NETLINK_LISTEN_ALL_NSID)?;
        Ok(res == 1)
    }

    /// `NETLINK_CAP_ACK` (since Linux 4.2). The kernel may fail to allocate the necessary room
    /// for the acknowledgment message back to user space.  This option trims off the payload of
    /// the original netlink message. The netlink message header is still included, so the user can
    /// guess from the sequence  number which message triggered the acknowledgment.
    pub fn set_cap_ack(&mut self, value: bool) -> Result<()> {
        let value: libc::c_int = if value { 1 } else { 0 };
        setsockopt(self.0, libc::SOL_NETLINK, libc::NETLINK_CAP_ACK, value)
    }

    pub fn get_cap_ack(&self) -> Result<bool> {
        let res = getsockopt::<libc::c_int>(self.0, libc::SOL_NETLINK, libc::NETLINK_CAP_ACK)?;
        Ok(res == 1)
    }
}

/// Wrapper around `getsockopt`:
///
/// ```no_rust
/// int getsockopt(int socket, int level, int option_name, void *restrict option_value, socklen_t *restrict option_len);
/// ```
pub(crate) fn getsockopt<T: Copy>(fd: RawFd, level: libc::c_int, option: libc::c_int) -> Result<T> {
    unsafe {
        // Create storage for the options we're fetching
        let mut slot: T = mem::zeroed();

        // Create a mutable raw pointer to the storage so that getsockopt can fill the value
        let slot_ptr = &mut slot as *mut T as *mut libc::c_void;

        // Let getsockopt know how big our storage is
        let mut slot_len = mem::size_of::<T>() as libc::socklen_t;

        // getsockopt takes a mutable pointer to the length, because for some options like
        // NETLINK_LIST_MEMBERSHIP where the option value is a list with arbitrary length,
        // getsockopt uses this parameter to signal how big the storage needs to be.
        let slot_len_ptr = &mut slot_len as *mut libc::socklen_t;

        let res = libc::getsockopt(fd, level, option, slot_ptr, slot_len_ptr);
        if res < 0 {
            return Err(Error::last_os_error());
        }

        // Ignore the options that require the legnth to be set by getsockopt.
        // We'll deal with them individually.
        assert_eq!(slot_len as usize, mem::size_of::<T>());

        Ok(slot)
    }
}

// adapted from rust standard library
fn setsockopt<T>(fd: RawFd, level: libc::c_int, option: libc::c_int, payload: T) -> Result<()> {
    unsafe {
        let payload = &payload as *const T as *const libc::c_void;
        let payload_len = mem::size_of::<T>() as libc::socklen_t;

        let res = libc::setsockopt(fd, level, option, payload, payload_len);
        if res < 0 {
            return Err(Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new() {
        Socket::new(Protocol::Route).unwrap();
    }

    #[test]
    fn connect() {
        let sock = Socket::new(Protocol::Route).unwrap();
        sock.connect(&SocketAddr::new(0, 0)).unwrap();
    }

    #[test]
    fn bind() {
        let mut sock = Socket::new(Protocol::Route).unwrap();
        sock.bind(&SocketAddr::new(4321, 0)).unwrap();
    }

    #[test]
    fn bind_auto() {
        let mut sock = Socket::new(Protocol::Route).unwrap();
        let addr = sock.bind_auto().unwrap();
        // make sure that the address we got from the kernel is there
        assert!(addr.port_number() != 0);
    }

    #[test]
    fn set_non_blocking() {
        let sock = Socket::new(Protocol::Route).unwrap();
        sock.set_non_blocking(true).unwrap();
        sock.set_non_blocking(false).unwrap();
    }

    #[test]
    fn options() {
        let mut sock = Socket::new(Protocol::Route).unwrap();

        sock.set_cap_ack(true).unwrap();
        assert!(sock.get_cap_ack().unwrap());
        sock.set_cap_ack(false).unwrap();
        assert!(!sock.get_cap_ack().unwrap());

        sock.set_no_enobufs(true).unwrap();
        assert!(sock.get_no_enobufs().unwrap());
        sock.set_no_enobufs(false).unwrap();
        assert!(!sock.get_no_enobufs().unwrap());

        sock.set_broadcast_error(true).unwrap();
        assert!(sock.get_broadcast_error().unwrap());
        sock.set_broadcast_error(false).unwrap();
        assert!(!sock.get_broadcast_error().unwrap());

        // FIXME: these require root permissions
        // sock.set_listen_all_namespaces(true).unwrap();
        // assert!(sock.get_listen_all_namespaces().unwrap());
        // sock.set_listen_all_namespaces(false).unwrap();
        // assert!(!sock.get_listen_all_namespaces().unwrap());
    }

    #[test]
    fn address() {
        let mut addr = SocketAddr::new(42, 1234);
        assert_eq!(addr.port_number(), 42);
        assert_eq!(addr.multicast_groups(), 1234);

        {
            let (addr_ptr, _) = addr.as_raw();
            let inner_addr = unsafe { *(addr_ptr as *const libc::sockaddr_nl) };
            assert_eq!(inner_addr.nl_pid, 42);
            assert_eq!(inner_addr.nl_groups, 1234);
        }

        {
            let (addr_ptr, _) = addr.as_raw_mut();
            let sockaddr_nl = addr_ptr as *mut libc::sockaddr_nl;
            unsafe {
                sockaddr_nl.as_mut().unwrap().nl_pid = 24;
                sockaddr_nl.as_mut().unwrap().nl_groups = 4321
            }
        }
        assert_eq!(addr.port_number(), 24);
        assert_eq!(addr.multicast_groups(), 4321);
    }
}
