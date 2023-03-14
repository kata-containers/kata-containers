// SPDX-License-Identifier: MIT

//! `netlink-proto` is an asynchronous implementation of the Netlink
//! protocol.
//!
//! # Example: listening for audit events
//!
//! This example shows how to use `netlink-proto` with the `tokio`
//! runtime to print audit events. It requires extra external
//! dependencies:
//!
//! - `futures = "^0.3"`
//! - `tokio = "^1.0"`
//! - `netlink-packet-audit = "^0.1"`
//!
//! ```rust,no_run
//! use futures::stream::StreamExt;
//! use netlink_packet_audit::{
//!     AuditMessage,
//!     NetlinkMessage,
//!     NetlinkPayload,
//!     StatusMessage,
//!     NLM_F_ACK,
//!     NLM_F_REQUEST,
//! };
//! use std::process;
//!
//! use netlink_proto::{
//!     new_connection,
//!     sys::{protocols::NETLINK_AUDIT, SocketAddr},
//! };
//!
//! const AUDIT_STATUS_ENABLED: u32 = 1;
//! const AUDIT_STATUS_PID: u32 = 4;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), String> {
//!     // Create a netlink socket. Here:
//!     //
//!     // - `conn` is a `Connection` that has the netlink socket. It's a
//!     //   `Future` that keeps polling the socket and must be spawned an
//!     //   the event loop.
//!     //
//!     // - `handle` is a `Handle` to the `Connection`. We use it to send
//!     //   netlink messages and receive responses to these messages.
//!     //
//!     // - `messages` is a channel receiver through which we receive
//!     //   messages that we have not solicited, ie that are not
//!     //   response to a request we made. In this example, we'll receive
//!     //   the audit event through that channel.
//!     let (conn, mut handle, mut messages) = new_connection(NETLINK_AUDIT)
//!         .map_err(|e| format!("Failed to create a new netlink connection: {}", e))?;
//!
//!     // Spawn the `Connection` so that it starts polling the netlink
//!     // socket in the background.
//!     tokio::spawn(conn);
//!
//!     // Use the `ConnectionHandle` to send a request to the kernel
//!     // asking it to start multicasting audit event messages.
//!     tokio::spawn(async move {
//!         // Craft the packet to enable audit events
//!         let mut status = StatusMessage::new();
//!         status.enabled = 1;
//!         status.pid = process::id();
//!         status.mask = AUDIT_STATUS_ENABLED | AUDIT_STATUS_PID;
//!         let payload = AuditMessage::SetStatus(status);
//!         let mut nl_msg = NetlinkMessage::from(payload);
//!         nl_msg.header.flags = NLM_F_REQUEST | NLM_F_ACK;
//!
//!         // We'll send unicast messages to the kernel.
//!         let kernel_unicast: SocketAddr = SocketAddr::new(0, 0);
//!         let mut response = match handle.request(nl_msg, kernel_unicast) {
//!             Ok(response) => response,
//!             Err(e) => {
//!                 eprintln!("{}", e);
//!                 return;
//!             }
//!         };
//!
//!         while let Some(message) = response.next().await {
//!             if let NetlinkPayload::Error(err_message) = message.payload {
//!                 eprintln!("Received an error message: {:?}", err_message);
//!                 return;
//!             }
//!         }
//!     });
//!
//!     // Finally, start receiving event through the `messages` channel.
//!     println!("Starting to print audit events... press ^C to interrupt");
//!     while let Some((message, _addr)) = messages.next().await {
//!         if let NetlinkPayload::Error(err_message) = message.payload {
//!             eprintln!("received an error message: {:?}", err_message);
//!         } else {
//!             println!("{:?}", message);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Example: dumping all the machine's links
//!
//! This example shows how to use `netlink-proto` with the ROUTE
//! protocol.
//!
//! Here we do not use `netlink_proto::new_connection()`, and instead
//! create the socket manually and use call `send()` and `receive()`
//! directly. In the previous example, the `NetlinkFramed` was wrapped
//! in a `Connection` which was polled automatically by the runtime.
//!
//! ```rust,no_run
//! use futures::StreamExt;
//!
//! use netlink_packet_route::{
//!     LinkMessage,
//!     NetlinkHeader,
//!     NetlinkMessage,
//!     RtnlMessage,
//!     NLM_F_DUMP,
//!     NLM_F_REQUEST,
//! };
//!
//! use netlink_proto::{
//!     new_connection,
//!     sys::{protocols::NETLINK_ROUTE, SocketAddr},
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<(), String> {
//!     // Create the netlink socket. Here, we won't use the channel that
//!     // receives unsolicited messages.
//!     let (conn, mut handle, _) = new_connection(NETLINK_ROUTE)
//!         .map_err(|e| format!("Failed to create a new netlink connection: {}", e))?;
//!
//!     // Spawn the `Connection` in the background
//!     tokio::spawn(conn);
//!
//!     // Create the netlink message that requests the links to be dumped
//!     let msg = NetlinkMessage {
//!         header: NetlinkHeader {
//!             sequence_number: 1,
//!             flags: NLM_F_DUMP | NLM_F_REQUEST,
//!             ..Default::default()
//!         },
//!         payload: RtnlMessage::GetLink(LinkMessage::default()).into(),
//!     };
//!
//!     // Send the request
//!     let mut response = handle
//!         .request(msg, SocketAddr::new(0, 0))
//!         .map_err(|e| format!("Failed to send request: {}", e))?;
//!
//!     // Print all the messages received in response
//!     loop {
//!         if let Some(packet) = response.next().await {
//!             println!("<<< {:?}", packet);
//!         } else {
//!             break;
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

mod codecs;
pub use crate::codecs::*;

mod framed;
pub use crate::framed::*;

mod protocol;
pub(crate) use self::protocol::{Protocol, Response};
pub(crate) type Request<T> =
    self::protocol::Request<T, UnboundedSender<crate::packet::NetlinkMessage<T>>>;

mod connection;
pub use crate::connection::*;

mod errors;
pub use crate::errors::*;

mod handle;
pub use crate::handle::*;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::{fmt::Debug, io};

pub use netlink_packet_core as packet;

pub mod sys {
    pub use netlink_sys::{protocols, AsyncSocket, AsyncSocketExt, SocketAddr};

    #[cfg(feature = "tokio_socket")]
    pub use netlink_sys::TokioSocket;

    #[cfg(feature = "smol_socket")]
    pub use netlink_sys::SmolSocket;
}

/// Create a new Netlink connection for the given Netlink protocol, and returns a handle to that
/// connection as well as a stream of unsolicited messages received by that connection (unsolicited
/// here means messages that are not a response to a request made by the `Connection`).
/// `Connection<T>` wraps a Netlink socket and implements the Netlink protocol.
///
/// `protocol` must be one of the [`crate::sys::protocols`][protos] constants.
///
/// `T` is the type of netlink messages used for this protocol. For instance, if you're using the
/// `NETLINK_AUDIT` protocol with the `netlink-packet-audit` crate, `T` will be
/// `netlink_packet_audit::AuditMessage`. More generally, `T` is anything that can be serialized
/// and deserialized into a Netlink message. See the `netlink_packet_core` documentation for
/// details about the `NetlinkSerializable` and `NetlinkDeserializable` traits.
///
/// Most of the time, users will want to spawn the `Connection` on an async runtime, and use the
/// handle to send messages.
///
/// [protos]: crate::sys::protocols
#[cfg(feature = "tokio_socket")]
#[allow(clippy::type_complexity)]
pub fn new_connection<T>(
    protocol: isize,
) -> io::Result<(
    Connection<T>,
    ConnectionHandle<T>,
    UnboundedReceiver<(packet::NetlinkMessage<T>, sys::SocketAddr)>,
)>
where
    T: Debug + packet::NetlinkSerializable + packet::NetlinkDeserializable + Unpin,
{
    new_connection_with_codec(protocol)
}

/// Variant of [`new_connection`] that allows specifying a socket type to use for async handling
#[allow(clippy::type_complexity)]
pub fn new_connection_with_socket<T, S>(
    protocol: isize,
) -> io::Result<(
    Connection<T, S>,
    ConnectionHandle<T>,
    UnboundedReceiver<(packet::NetlinkMessage<T>, sys::SocketAddr)>,
)>
where
    T: Debug + packet::NetlinkSerializable + packet::NetlinkDeserializable + Unpin,
    S: sys::AsyncSocket,
{
    new_connection_with_codec(protocol)
}

/// Variant of [`new_connection`] that allows specifying a socket type to use for async handling and a special codec
#[allow(clippy::type_complexity)]
pub fn new_connection_with_codec<T, S, C>(
    protocol: isize,
) -> io::Result<(
    Connection<T, S, C>,
    ConnectionHandle<T>,
    UnboundedReceiver<(packet::NetlinkMessage<T>, sys::SocketAddr)>,
)>
where
    T: Debug + packet::NetlinkSerializable + packet::NetlinkDeserializable + Unpin,
    S: sys::AsyncSocket,
    C: NetlinkMessageCodec,
{
    let (requests_tx, requests_rx) = unbounded::<Request<T>>();
    let (messages_tx, messages_rx) = unbounded::<(packet::NetlinkMessage<T>, sys::SocketAddr)>();
    Ok((
        Connection::new(requests_rx, messages_tx, protocol)?,
        ConnectionHandle::new(requests_tx),
        messages_rx,
    ))
}
