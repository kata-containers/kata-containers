// SPDX-License-Identifier: MIT

//! `netlink-packet-core` provides a generic netlink message
//! `NetlinkMessage<T>` that is independant of the sub-protocol. Such
//! messages are not very useful by themselves, since they are just
//! used to carry protocol-dependant messages. That is what the `T`
//! represent: `T` is the `NetlinkMessage`'s protocol-dependant
//! message. This can be any type that implements
//! `NetlinkSerializable` and `NetlinkDeserializable`.
//!
//! For instance, the `netlink-packet-route` crate provides rtnetlink
//! messages via `netlink_packet_route::RtnlMessage`, and
//! `netlink-packet-audit` provides audit messages via
//! `netlink_packet_audit::AuditMessage`.
//!
//! By itself, the `netlink-packet-core` crate is not very
//! useful. However, it is used in `netlink-proto` to provide an
//! asynchronous implementation of the netlink protocol for any
//! sub-protocol. Thus, a crate that defines messages for a given
//! netlink sub-protocol could integrate with `netlink-packet-core`
//! and would get an asynchronous implementation for free. See the
//! second example below for such an integration, via the
//! `NetlinkSerializable` and `NetlinkDeserializable` traits.
//!
//! # Example: usage with `netlink-packet-route`
//!
//! This example shows how to serialize and deserialize netlink packet
//! for the rtnetlink sub-protocol. It requires
//! `netlink-packet-route`.
//!
//! ```rust
//! use netlink_packet_core::{NetlinkHeader, NetlinkMessage, NLM_F_DUMP, NLM_F_REQUEST};
//! use netlink_packet_route::{LinkMessage, RtnlMessage};
//!
//! // Create the netlink message, that contains the rtnetlink
//! // message
//! let mut packet = NetlinkMessage {
//!     header: NetlinkHeader {
//!         sequence_number: 1,
//!         flags: NLM_F_DUMP | NLM_F_REQUEST,
//!         ..Default::default()
//!     },
//!     payload: RtnlMessage::GetLink(LinkMessage::default()).into(),
//! };
//!
//! // Before serializing the packet, it is important to call
//! // finalize() to ensure the header of the message is consistent
//! // with its payload. Otherwise, a panic may occur when calling
//! // serialize()
//! packet.finalize();
//!
//! // Prepare a buffer to serialize the packet. Note that we never
//! // set explicitely `packet.header.length` above. This was done
//! // automatically when we called `finalize()`
//! let mut buf = vec![0; packet.header.length as usize];
//! // Serialize the packet
//! packet.serialize(&mut buf[..]);
//!
//! // Deserialize the packet
//! let deserialized_packet =
//!     NetlinkMessage::<RtnlMessage>::deserialize(&buf).expect("Failed to deserialize message");
//!
//! // Normally, the deserialized packet should be exactly the same
//! // than the serialized one.
//! assert_eq!(deserialized_packet, packet);
//!
//! println!("{:?}", packet);
//! ```
//!
//! # Example: adding messages for new netlink sub-protocol
//!
//! Let's assume we have a netlink protocol called "ping pong" that
//! defines two types of messages: "ping" messages, which payload can
//! be any sequence of bytes, and "pong" message, which payload is
//! also a sequence of bytes.  The protocol works as follow: when an
//! enpoint receives a "ping" message, it answers with a "pong", with
//! the payload of the "ping" it's answering to.
//!
//! "ping" messages have type 18 and "pong" have type "20". Here is
//! what a "ping" message that would look like if its payload is `[0,
//! 1, 2, 3]`:
//!
//! ```no_rust
//! 0                8                16              24               32
//! +----------------+----------------+----------------+----------------+
//! |                 packet length (including header) = 16 + 4 = 20    |
//! +----------------+----------------+----------------+----------------+
//! |     message type = 18 (ping)    |              flags              |
//! +----------------+----------------+----------------+----------------+
//! |                           sequence number                         |
//! +----------------+----------------+----------------+----------------+
//! |                            port number                            |
//! +----------------+----------------+----------------+----------------+
//! |       0        |         1      |         2      |        3       |
//! +----------------+----------------+----------------+----------------+
//! ```
//!
//! And the "pong" response would be:
//!
//! ```no_rust
//! 0                8                16              24               32
//! +----------------+----------------+----------------+----------------+
//! |                 packet length (including header) = 16 + 4 = 20    |
//! +----------------+----------------+----------------+----------------+
//! |     message type = 20 (pong)    |              flags              |
//! +----------------+----------------+----------------+----------------+
//! |                           sequence number                         |
//! +----------------+----------------+----------------+----------------+
//! |                            port number                            |
//! +----------------+----------------+----------------+----------------+
//! |       0        |         1      |         2      |        3       |
//! +----------------+----------------+----------------+----------------+
//! ```
//!
//! Here is how we could implement the messages for such a protocol
//! and integrate this implementation with `netlink-packet-core`:
//!
//! ```rust
//! use netlink_packet_core::{
//!     NetlinkDeserializable, NetlinkHeader, NetlinkMessage, NetlinkPayload, NetlinkSerializable,
//! };
//! use std::error::Error;
//! use std::fmt;
//!
//! // PingPongMessage represent the messages for the "ping-pong" netlink
//! // protocol. There are only two types of messages.
//! #[derive(Debug, Clone, Eq, PartialEq)]
//! pub enum PingPongMessage {
//!     Ping(Vec<u8>),
//!     Pong(Vec<u8>),
//! }
//!
//! // The netlink header contains a "message type" field that identifies
//! // the message it carries. Some values are reserved, and we
//! // arbitrarily decided that "ping" type is 18 and "pong" type is 20.
//! pub const PING_MESSAGE: u16 = 18;
//! pub const PONG_MESSAGE: u16 = 20;
//!
//! // A custom error type for when deserialization fails. This is
//! // required because `NetlinkDeserializable::Error` must implement
//! // `std::error::Error`, so a simple `String` won't cut it.
//! #[derive(Debug, Clone, Eq, PartialEq)]
//! pub struct DeserializeError(&'static str);
//!
//! impl Error for DeserializeError {
//!     fn description(&self) -> &str {
//!         self.0
//!     }
//!     fn source(&self) -> Option<&(dyn Error + 'static)> {
//!         None
//!     }
//! }
//!
//! impl fmt::Display for DeserializeError {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         write!(f, "{}", self.0)
//!     }
//! }
//!
//! // NetlinkDeserializable implementation
//! impl NetlinkDeserializable for PingPongMessage {
//!     type Error = DeserializeError;
//!
//!     fn deserialize(header: &NetlinkHeader, payload: &[u8]) -> Result<Self, Self::Error> {
//!         match header.message_type {
//!             PING_MESSAGE => Ok(PingPongMessage::Ping(payload.to_vec())),
//!             PONG_MESSAGE => Ok(PingPongMessage::Pong(payload.to_vec())),
//!             _ => Err(DeserializeError(
//!                 "invalid ping-pong message: invalid message type",
//!             )),
//!         }
//!     }
//! }
//!
//! // NetlinkSerializable implementation
//! impl NetlinkSerializable for PingPongMessage {
//!     fn message_type(&self) -> u16 {
//!         match self {
//!             PingPongMessage::Ping(_) => PING_MESSAGE,
//!             PingPongMessage::Pong(_) => PONG_MESSAGE,
//!         }
//!     }
//!
//!     fn buffer_len(&self) -> usize {
//!         match self {
//!             PingPongMessage::Ping(vec) | PingPongMessage::Pong(vec) => vec.len(),
//!         }
//!     }
//!
//!     fn serialize(&self, buffer: &mut [u8]) {
//!         match self {
//!             PingPongMessage::Ping(vec) | PingPongMessage::Pong(vec) => {
//!                 buffer.copy_from_slice(&vec[..])
//!             }
//!         }
//!     }
//! }
//!
//! // It can be convenient to be able to create a NetlinkMessage directly
//! // from a PingPongMessage. Since NetlinkMessage<T> already implements
//! // From<NetlinkPayload<T>>, we just need to implement
//! // From<NetlinkPayload<PingPongMessage>> for this to work.
//! impl From<PingPongMessage> for NetlinkPayload<PingPongMessage> {
//!     fn from(message: PingPongMessage) -> Self {
//!         NetlinkPayload::InnerMessage(message)
//!     }
//! }
//!
//! fn main() {
//!     let ping_pong_message = PingPongMessage::Ping(vec![0, 1, 2, 3]);
//!     let mut packet = NetlinkMessage::from(ping_pong_message);
//!
//!     // Before serializing the packet, it is very important to call
//!     // finalize() to ensure the header of the message is consistent
//!     // with its payload. Otherwise, a panic may occur when calling
//!     // `serialize()`
//!     packet.finalize();
//!
//!     // Prepare a buffer to serialize the packet. Note that we never
//!     // set explicitely `packet.header.length` above. This was done
//!     // automatically when we called `finalize()`
//!     let mut buf = vec![0; packet.header.length as usize];
//!     // Serialize the packet
//!     packet.serialize(&mut buf[..]);
//!
//!     // Deserialize the packet
//!     let deserialized_packet = NetlinkMessage::<PingPongMessage>::deserialize(&buf)
//!         .expect("Failed to deserialize message");
//!
//!     // Normally, the deserialized packet should be exactly the same
//!     // than the serialized one.
//!     assert_eq!(deserialized_packet, packet);
//!
//!     // This should print:
//!     // NetlinkMessage { header: NetlinkHeader { length: 20, message_type: 18, flags: 0, sequence_number: 0, port_number: 0 }, payload: InnerMessage(Ping([0, 1, 2, 3])) }
//!     println!("{:?}", packet);
//! }
//! ```

use core::ops::{Range, RangeFrom};
/// Represent a multi-bytes field with a fixed size in a packet
pub(crate) type Field = Range<usize>;
/// Represent a field that starts at a given index in a packet
pub(crate) type Rest = RangeFrom<usize>;

pub mod error;
pub use self::error::*;

pub mod buffer;
pub use self::buffer::*;

pub mod header;
pub use self::header::*;

mod traits;
pub use self::traits::*;

mod payload;
pub use self::payload::*;

mod message;
pub use self::message::*;

pub mod constants;
pub use self::constants::*;

pub use self::utils::errors::*;
pub(crate) use self::utils::traits::*;
pub(crate) use netlink_packet_utils as utils;
