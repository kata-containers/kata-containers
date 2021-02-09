use std::fmt::Debug;

use netlink_packet_core::NetlinkMessage;

use crate::sys::SocketAddr;

#[derive(Debug)]
pub struct Request<T, M>
where
    T: Debug + Clone + Eq + PartialEq,
    M: Debug,
{
    pub metadata: M,
    pub message: NetlinkMessage<T>,
    pub destination: SocketAddr,
}

impl<T, M> From<(NetlinkMessage<T>, SocketAddr, M)> for Request<T, M>
where
    T: Debug + PartialEq + Eq + Clone,
    M: Debug,
{
    fn from(parts: (NetlinkMessage<T>, SocketAddr, M)) -> Self {
        Request {
            message: parts.0,
            destination: parts.1,
            metadata: parts.2,
        }
    }
}

impl<T, M> Into<(NetlinkMessage<T>, SocketAddr, M)> for Request<T, M>
where
    T: Debug + PartialEq + Eq + Clone,
    M: Debug,
{
    fn into(self) -> (NetlinkMessage<T>, SocketAddr, M) {
        (self.message, self.destination, self.metadata)
    }
}
