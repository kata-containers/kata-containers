// SPDX-License-Identifier: MIT

use std::io;

use netlink_packet_core::NetlinkMessage;

#[derive(thiserror::Error, Debug)]
pub enum Error<T> {
    /// The netlink connection is closed
    #[error("the netlink connection is closed")]
    ConnectionClosed,

    /// Received an error message as a response
    #[error("received an error message as a response: {0:?}")]
    NetlinkError(NetlinkMessage<T>),

    /// Error while reading from or writing to the netlink socket
    #[error("error while reading from or writing to the netlink socket: {0}")]
    SocketIo(#[from] io::Error),
}
