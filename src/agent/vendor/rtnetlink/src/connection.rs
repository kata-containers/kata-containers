use std::io;

use futures::channel::mpsc::UnboundedReceiver;

use crate::{
    packet::{NetlinkMessage, RtnlMessage},
    proto::Connection,
    sys::{protocols::NETLINK_ROUTE, SocketAddr},
    Handle,
};

#[allow(clippy::type_complexity)]
pub fn new_connection() -> io::Result<(
    Connection<RtnlMessage>,
    Handle,
    UnboundedReceiver<(NetlinkMessage<RtnlMessage>, SocketAddr)>,
)> {
    let (conn, handle, messages) = netlink_proto::new_connection(NETLINK_ROUTE)?;
    Ok((conn, Handle::new(handle), messages))
}
