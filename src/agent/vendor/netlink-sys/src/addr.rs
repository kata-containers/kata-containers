use std::{
    fmt,
    hash::{Hash, Hasher},
    mem,
};

/// The address of a netlink socket
///
/// A netlink address is made of two parts: the unicast address of the socket, called _port number_ or _PID_, and the
/// multicast address called _group ID_. In this library, we've chosen to stick to the "port number" terminology, since
/// PID can be confused with process ID. However, the netlink man page mostly uses PID.
///
/// ## Port number
///
/// Sockets in kernel space have 0 as a port number. For sockets opened by a user-space process, the port number can
/// either be assigned by the process itself, or by the kernel. The only constraint is that this port number must be
/// unique: two netlink sockets created by a given process must have a different port number. However, netlinks sockets
/// created by different processes can have the same port number.
///
/// ### Port number assigned by the kernel
///
/// One way to set the port number is to let the kernel assign it, by calling [`Socket::bind`][bind] with a port number set to
/// 0. The kernel will usually use the process ID as port number for the first netlink socket created by the process,
/// which is why the socket port number is also called PID. For example:
///
/// ```rust
/// use std::process;
/// use netlink_sys::{
///     protocols::NETLINK_ROUTE,
///     SocketAddr, Socket,
/// };
///
/// let mut socket = Socket::new(NETLINK_ROUTE).unwrap();
/// // The first parameter is the port number. By setting it to 0 we ask the kernel to pick a port for us
/// let mut addr = SocketAddr::new(0, 0);
/// socket.bind(&addr).unwrap();
/// // Retrieve the socket address
/// socket.get_address(&mut addr).unwrap();
/// // the socket port number should be equal to the process ID, but there is no guarantee
/// println!("socket port number = {}, process ID = {}", addr.port_number(), process::id());
///
/// let mut socket2 = Socket::new(NETLINK_ROUTE).unwrap();
/// let mut addr2 = SocketAddr::new(0, 0);
/// socket2.bind(&addr2).unwrap();
/// socket2.get_address(&mut addr2).unwrap();
/// // the unicast address picked by the kernel for the second socket should be different
/// assert!(addr.port_number() != addr2.port_number());
/// ```
///
/// Note that it's a little tedious to create a socket address, call `bind` and then retrive the address with
/// [`Socket::get_address`][get_addr]. To avoid this boilerplate you can use [`Socket::bind_auto`][bind_auto]:
///
/// ```rust
/// use netlink_sys::{protocols::NETLINK_ROUTE, Socket, SocketAddr};
/// use std::process;
///
/// let mut socket = Socket::new(NETLINK_ROUTE).unwrap();
/// let addr = socket.bind_auto().unwrap();
/// println!("socket port number = {}", addr.port_number());
/// ```
///
/// ### Setting the port number manually
///
/// The application can also pick the port number by calling Socket::bind with an address with a non-zero port
/// number. However, it must ensure that this number is unique for each socket created. For instance:
///
/// ```rust
/// use netlink_sys::{protocols::NETLINK_ROUTE, Socket, SocketAddr};
/// use std::process;
///
/// let mut socket = Socket::new(NETLINK_ROUTE).unwrap();
/// // set the socket port number to 2
/// let mut addr = SocketAddr::new(2, 0);
/// socket.bind(&addr).unwrap();
/// // Retrieve the socket address
/// socket.get_address(&mut addr).unwrap();
/// assert_eq!(2, addr.port_number());
///
/// // Creating a second socket with the same port number fails
/// let mut socket2 = Socket::new(NETLINK_ROUTE).unwrap();
/// let mut addr2 = SocketAddr::new(2, 0);
/// socket2.bind(&addr2).unwrap_err();
/// ```
///
/// [bind]: crate::Socket::bind
/// [bind_auto]: crate::Socket::bind_auto
/// [get_addr]: crate::Socket::get_address
#[derive(Copy, Clone)]
pub struct SocketAddr(pub(crate) libc::sockaddr_nl);

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
    /// Create a new socket address for with th
    pub fn new(port_number: u32, multicast_groups: u32) -> Self {
        let mut addr: libc::sockaddr_nl = unsafe { mem::zeroed() };
        addr.nl_family = libc::PF_NETLINK as libc::sa_family_t;
        addr.nl_pid = port_number;
        addr.nl_groups = multicast_groups;
        SocketAddr(addr)
    }

    /// Get the unicast address of this socket
    pub fn port_number(&self) -> u32 {
        self.0.nl_pid
    }

    /// Get the multicast groups of this socket
    pub fn multicast_groups(&self) -> u32 {
        self.0.nl_groups
    }

    pub(crate) fn as_raw(&self) -> (*const libc::sockaddr, libc::socklen_t) {
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

    pub(crate) fn as_raw_mut(&mut self) -> (*mut libc::sockaddr, libc::socklen_t) {
        let addr_ptr = &mut self.0 as *mut libc::sockaddr_nl as *mut libc::sockaddr;
        let addr_len = mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        (addr_ptr, addr_len)
    }
}
