/// Receives routing and link updates and may be used to modify the routing tables (both IPv4
/// and IPv6), IP addresses, link parameters, neighbor setups, queueing disciplines, traffic
/// classes  and  packet  classifiers  (see rtnetlink(7)).
pub const NETLINK_ROUTE: isize = 0;
pub const NETLINK_UNUSED: isize = 1;
/// Reserved for user-mode socket protocols.
pub const NETLINK_USERSOCK: isize = 2;
/// Transport  IPv4  packets  from  netfilter  to  user  space.  Used by ip_queue kernel
/// module.  After a long period of being declared obsolete (in favor of the more advanced
/// nfnetlink_queue feature), it was  removed in Linux 3.5.
pub const NETLINK_FIREWALL: isize = 3;
/// Query information about sockets of various protocol families from the kernel (see sock_diag(7)).
pub const NETLINK_SOCK_DIAG: isize = 4;
/// Netfilter/iptables ULOG.
pub const NETLINK_NFLOG: isize = 5;
/// IPsec.
pub const NETLINK_XFRM: isize = 6;
/// SELinux event notifications.
pub const NETLINK_SELINUX: isize = 7;
/// Open-iSCSI.
pub const NETLINK_ISCSI: isize = 8;
/// Auditing.
pub const NETLINK_AUDIT: isize = 9;
/// Access to FIB lookup from user space.
pub const NETLINK_FIB_LOOKUP: isize = 10;
/// Kernel connector. See `Documentation/connector/*` in the Linux kernel source tree for further information.
pub const NETLINK_CONNECTOR: isize = 11;
/// Netfilter subsystem.
pub const NETLINK_NETFILTER: isize = 12;
/// Transport IPv6 packets from netfilter to user space.  Used by ip6_queue kernel module.
pub const NETLINK_IP6_FW: isize = 13;
/// DECnet routing messages.
pub const NETLINK_DNRTMSG: isize = 14;
/// Kernel messages to user space.
pub const NETLINK_KOBJECT_UEVENT: isize = 15;
///  Generic netlink family for simplified netlink usage.
pub const NETLINK_GENERIC: isize = 16;
/// SCSI transpots
pub const NETLINK_SCSITRANSPORT: isize = 18;
///
pub const NETLINK_ECRYPTFS: isize = 19;
/// Infiniband RDMA.
pub const NETLINK_RDMA: isize = 20;
/// Netlink interface to request information about ciphers registered with the kernel crypto
/// API as well as allow configuration of the kernel crypto API.
pub const NETLINK_CRYPTO: isize = 21;

/// List of netlink protocols
pub enum Protocol {
    /// Receives routing and link updates and may be used to modify the routing tables (both IPv4
    /// and IPv6), IP addresses, link parameters, neighbor setups, queueing disciplines, traffic
    /// classes  and  packet  classifiers  (see rtnetlink(7)).
    Route = NETLINK_ROUTE,
    Unused = NETLINK_UNUSED,
    /// Reserved for user-mode socket protocols.
    UserSock = NETLINK_USERSOCK,
    /// Transport  IPv4  packets  from  netfilter  to  user  space.  Used by ip_queue kernel
    /// module.  After a long period of being declared obsolete (in favor of the more advanced
    /// nfnetlink_queue feature), it was  removed in Linux 3.5.
    Firewall = NETLINK_FIREWALL,
    /// Query information about sockets of various protocol families from the kernel (see sock_diag(7)).
    SockDiag = NETLINK_SOCK_DIAG,
    /// Netfilter/iptables ULOG.
    NfLog = NETLINK_NFLOG,
    /// IPsec.
    Xfrm = NETLINK_XFRM,
    /// SELinux event notifications.
    SELinux = NETLINK_SELINUX,
    /// Open-iSCSI.
    ISCSI = NETLINK_ISCSI,
    /// Auditing.
    Audit = NETLINK_AUDIT,
    /// Access to FIB lookup from user space.
    FibLookup = NETLINK_FIB_LOOKUP,
    /// Kernel connector. See `Documentation/connector/*` in the Linux kernel source tree for further information.
    Connector = NETLINK_CONNECTOR,
    /// Netfilter subsystem.
    Netfilter = NETLINK_NETFILTER,
    /// Transport IPv6 packets from netfilter to user space.  Used by ip6_queue kernel module.
    Ip6Fw = NETLINK_IP6_FW,
    /// DECnet routing messages.
    Decnet = NETLINK_DNRTMSG,
    /// Kernel messages to user space.
    KObjectUevent = NETLINK_KOBJECT_UEVENT,
    ///  Generic netlink family for simplified netlink usage.
    Generic = NETLINK_GENERIC,
    /// SCSI transpots
    ScsiTransport = NETLINK_SCSITRANSPORT,
    ///
    Ecryptfs = NETLINK_ECRYPTFS,
    /// Infiniband RDMA.
    Rdma = NETLINK_RDMA,
    /// Netlink interface to request information about ciphers registered with the kernel crypto
    /// API as well as allow configuration of the kernel crypto API.
    Crypto = NETLINK_CRYPTO,
}
