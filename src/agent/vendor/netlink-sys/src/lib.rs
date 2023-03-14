pub mod constants;
pub mod protocols {
    pub use super::constants::{
        NETLINK_AUDIT,
        NETLINK_CONNECTOR,
        NETLINK_CRYPTO,
        NETLINK_DNRTMSG,
        NETLINK_ECRYPTFS,
        NETLINK_FIB_LOOKUP,
        NETLINK_FIREWALL,
        NETLINK_GENERIC,
        NETLINK_IP6_FW,
        NETLINK_ISCSI,
        NETLINK_KOBJECT_UEVENT,
        NETLINK_NETFILTER,
        NETLINK_NFLOG,
        NETLINK_RDMA,
        NETLINK_ROUTE,
        NETLINK_SCSITRANSPORT,
        NETLINK_SELINUX,
        NETLINK_SOCK_DIAG,
        NETLINK_UNUSED,
        NETLINK_USERSOCK,
        NETLINK_XFRM,
    };
}

mod socket;
pub use self::socket::Socket;

mod addr;
pub use self::addr::SocketAddr;

#[cfg(feature = "tokio_socket")]
mod tokio;
#[cfg(feature = "tokio_socket")]
pub use self::tokio::TokioSocket;

#[cfg(feature = "smol_socket")]
mod smol;
#[cfg(feature = "smol_socket")]
pub use self::smol::SmolSocket;

#[cfg(feature = "mio_socket")]
mod mio;
