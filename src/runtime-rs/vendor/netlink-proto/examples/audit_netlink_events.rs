// SPDX-License-Identifier: MIT

// This example shows how to use `netlink-proto` with the tokio runtime to print audit events.
//
// This example shows how the netlink socket can be accessed
// `netlink_proto::Connection`, and configured (in this case to
// register to a multicast group).
//
// Compilation:
// ------------
//
// cargo build --example audit_events
//
// Usage:
// ------
//
// Find the example binary in the target directory, and run it *as
// root*. If you compiled in debug mode with the command above, the
// binary should be under:
// `<repo-root>/target/debug/examples/audit_events`. This example runs
// forever, you must hit ^C to kill it.

use futures::stream::StreamExt;
use netlink_packet_audit::{
    AuditMessage,
    NetlinkMessage,
    NetlinkPayload,
    StatusMessage,
    NLM_F_ACK,
    NLM_F_REQUEST,
};
use std::process;

use netlink_proto::{
    new_connection,
    sys::{protocols::NETLINK_AUDIT, SocketAddr},
};

const AUDIT_STATUS_ENABLED: u32 = 1;
const AUDIT_STATUS_PID: u32 = 4;

#[tokio::main]
async fn main() -> Result<(), String> {
    // Create a netlink socket. Here:
    //
    // - `conn` is a `Connection` that has the netlink socket. It's a
    //   `Future` that keeps polling the socket and must be spawned an
    //   the event loop.
    //
    // - `handle` is a `Handle` to the `Connection`. We use it to send
    //   netlink messages and receive responses to these messages.
    //
    // - `messages` is a channel receiver through which we receive
    //   messages that we have not sollicated, ie that are not
    //   response to a request we made. In this example, we'll receive
    //   the audit event through that channel.
    let (conn, mut handle, mut messages) = new_connection(NETLINK_AUDIT)
        .map_err(|e| format!("Failed to create a new netlink connection: {}", e))?;

    // Spawn the `Connection` so that it starts polling the netlink
    // socket in the background.
    tokio::spawn(conn);

    // Use the `ConnectionHandle` to send a request to the kernel
    // asking it to start multicasting audit event messages.
    tokio::spawn(async move {
        // Craft the packet to enable audit events
        let mut status = StatusMessage::new();
        status.enabled = 1;
        status.pid = process::id();
        status.mask = AUDIT_STATUS_ENABLED | AUDIT_STATUS_PID;
        let payload = AuditMessage::SetStatus(status);
        let mut nl_msg = NetlinkMessage::from(payload);
        nl_msg.header.flags = NLM_F_REQUEST | NLM_F_ACK;

        // We'll send unicast messages to the kernel.
        let kernel_unicast: SocketAddr = SocketAddr::new(0, 0);
        let mut response = match handle.request(nl_msg, kernel_unicast) {
            Ok(response) => response,
            Err(e) => {
                eprintln!("{}", e);
                return;
            }
        };

        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err_message) = message.payload {
                eprintln!("Received an error message: {:?}", err_message);
                return;
            }
        }
    });

    // Finally, start receiving event through the `messages` channel.
    println!("Starting to print audit events... press ^C to interrupt");
    while let Some((message, _addr)) = messages.next().await {
        if let NetlinkPayload::Error(err_message) = message.payload {
            eprintln!("received an error message: {:?}", err_message);
        } else {
            println!("{:?}", message);
        }
    }

    Ok(())
}
