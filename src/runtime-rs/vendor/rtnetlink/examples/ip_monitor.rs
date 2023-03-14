// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;

use netlink_packet_route::constants::*;
use rtnetlink::{
    new_connection,
    sys::{AsyncSocket, SocketAddr},
};

#[tokio::main]
async fn main() -> Result<(), String> {
    // conn - `Connection` that has a netlink socket which is a `Future` that polls the socket
    // and thus must have an event loop
    //
    // handle - `Handle` to the `Connection`. Used to send/recv netlink messages.
    //
    // messages - A channel receiver.
    let (mut conn, mut _handle, mut messages) = new_connection().map_err(|e| format!("{}", e))?;

    // These flags specify what kinds of broadcast messages we want to listen for.
    let groups = RTNLGRP_LINK
        | RTNLGRP_IPV4_IFADDR
        | RTNLGRP_IPV6_IFADDR
        | RTNLGRP_IPV4_ROUTE
        | RTNLGRP_IPV6_ROUTE
        | RTNLGRP_MPLS_ROUTE
        | RTNLGRP_IPV4_MROUTE
        | RTNLGRP_IPV6_MROUTE
        | RTNLGRP_NEIGH
        | RTNLGRP_IPV4_NETCONF
        | RTNLGRP_IPV6_NETCONF
        | RTNLGRP_IPV4_RULE
        | RTNLGRP_IPV6_RULE
        | RTNLGRP_NSID
        | RTNLGRP_MPLS_NETCONF;

    let addr = SocketAddr::new(0, groups);
    conn.socket_mut()
        .socket_mut()
        .bind(&addr)
        .expect("Failed to bind");

    // Spawn `Connection` to start polling netlink socket.
    tokio::spawn(conn);

    // Use `Handle` to send request to kernel to start multicasting rtnetlink events.
    tokio::spawn(async move {
        // Create message to enable
    });

    // Start receiving events through `messages` channel.
    while let Some((message, _)) = messages.next().await {
        let payload = message.payload;
        println!("{:?}", payload);
    }
    Ok(())
}
