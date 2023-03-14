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

impl<T, M> From<Request<T, M>> for (NetlinkMessage<T>, SocketAddr, M)
where
    T: Debug + PartialEq + Eq + Clone,
    M: Debug,
{
    fn from(req: Request<T, M>) -> (NetlinkMessage<T>, SocketAddr, M) {
        (req.message, req.destination, req.metadata)
    }
}
