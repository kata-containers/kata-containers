//! This module provides a lot of netlink constants for various protocol. As we add support for the
//! various protocols, these constants will be moved to their own crate.

use libc::c_int as int;

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
pub const NETLINK_ECRYPTFS: isize = 19;
/// Infiniband RDMA.
pub const NETLINK_RDMA: isize = 20;
/// Netlink interface to request information about ciphers registered with the kernel crypto
/// API as well as allow configuration of the kernel crypto API.
pub const NETLINK_CRYPTO: isize = 21;

pub const TCA_ROOT_UNSPEC: int = 0;
pub const TCA_ROOT_TAB: int = 1;
pub const TCA_ROOT_FLAGS: int = 2;
pub const TCA_ROOT_COUNT: int = 3;
pub const TCA_ROOT_TIME_DELTA: int = 4;

pub const EM_NONE: u32 = 0;
pub const EM_M32: u32 = 1;
pub const EM_SPARC: u32 = 2;
pub const EM_386: u32 = 3;
pub const EM_68K: u32 = 4;
pub const EM_88K: u32 = 5;
pub const EM_486: u32 = 6;
pub const EM_860: u32 = 7;
pub const EM_MIPS: u32 = 8;
pub const EM_MIPS_RS3_LE: u32 = 10;
pub const EM_MIPS_RS4_BE: u32 = 10;
pub const EM_PARISC: u32 = 15;
pub const EM_SPARC32PLUS: u32 = 18;
pub const EM_PPC: u32 = 20;
pub const EM_PPC64: u32 = 21;
pub const EM_SPU: u32 = 23;
pub const EM_ARM: u32 = 40;
pub const EM_SH: u32 = 42;
pub const EM_SPARCV9: u32 = 43;
pub const EM_H8_300: u32 = 46;
pub const EM_IA_64: u32 = 50;
pub const EM_X86_64: u32 = 62;
pub const EM_S390: u32 = 22;
pub const EM_CRIS: u32 = 76;
pub const EM_M32R: u32 = 88;
pub const EM_MN10300: u32 = 89;
pub const EM_OPENRISC: u32 = 92;
pub const EM_BLACKFIN: u32 = 106;
pub const EM_ALTERA_NIOS2: u32 = 113;
pub const EM_TI_C6000: u32 = 140;
pub const EM_AARCH64: u32 = 183;
pub const EM_TILEPRO: u32 = 188;
pub const EM_MICROBLAZE: u32 = 189;
pub const EM_TILEGX: u32 = 191;
pub const EM_BPF: u32 = 247;
pub const EM_FRV: u32 = 21569;
pub const EM_ALPHA: u32 = 36902;
pub const EM_CYGNUS_M32R: u32 = 36929;
pub const EM_S390_OLD: u32 = 41872;
pub const EM_CYGNUS_MN10300: u32 = 48879;

pub const NLMSGERR_ATTR_UNUSED: int = 0;
pub const NLMSGERR_ATTR_MSG: int = 1;
pub const NLMSGERR_ATTR_OFFS: int = 2;
pub const NLMSGERR_ATTR_COOKIE: int = 3;
pub const NLMSGERR_ATTR_MAX: int = 3;

pub const NL_MMAP_STATUS_UNUSED: int = 0;
pub const NL_MMAP_STATUS_RESERVED: int = 1;
pub const NL_MMAP_STATUS_VALID: int = 2;
pub const NL_MMAP_STATUS_COPY: int = 3;
pub const NL_MMAP_STATUS_SKIP: int = 4;

pub const NETLINK_UNCONNECTED: int = 0;
pub const NETLINK_CONNECTED: int = 1;

pub const __BITS_PER_LONG: int = 64;
pub const __FD_SETSIZE: int = 1024;
pub const SI_LOAD_SHIFT: int = 16;
pub const _K_SS_MAXSIZE: int = 128;
pub const NETLINK_SMC: int = 22;
pub const NETLINK_INET_DIAG: int = 4;
pub const MAX_LINKS: int = 32;

pub const NLMSG_MIN_TYPE: int = 16;
pub const NETLINK_ADD_MEMBERSHIP: int = 1;
pub const NETLINK_DROP_MEMBERSHIP: int = 2;
pub const NETLINK_PKTINFO: int = 3;
pub const NETLINK_BROADCAST_ERROR: int = 4;
pub const NETLINK_NO_ENOBUFS: int = 5;
pub const NETLINK_RX_RING: int = 6;
pub const NETLINK_TX_RING: int = 7;
pub const NETLINK_LISTEN_ALL_NSID: int = 8;
pub const NETLINK_LIST_MEMBERSHIPS: int = 9;
pub const NETLINK_CAP_ACK: int = 10;
pub const NETLINK_EXT_ACK: int = 11;
pub const NL_MMAP_MSG_ALIGNMENT: int = 4;
pub const NET_MAJOR: int = 36;
