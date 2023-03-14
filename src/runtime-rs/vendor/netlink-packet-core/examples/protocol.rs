// SPDX-License-Identifier: MIT

use std::{error::Error, fmt};

use netlink_packet_core::{
    NetlinkDeserializable,
    NetlinkHeader,
    NetlinkMessage,
    NetlinkPayload,
    NetlinkSerializable,
};

// PingPongMessage represent the messages for the "ping-pong" netlink
// protocol. There are only two types of messages.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PingPongMessage {
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

// The netlink header contains a "message type" field that identifies
// the message it carries. Some values are reserved, and we
// arbitrarily decided that "ping" type is 18 and "pong" type is 20.
pub const PING_MESSAGE: u16 = 18;
pub const PONG_MESSAGE: u16 = 20;

// A custom error type for when deserialization fails. This is
// required because `NetlinkDeserializable::Error` must implement
// `std::error::Error`, so a simple `String` won't cut it.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeserializeError(&'static str);

impl Error for DeserializeError {
    fn description(&self) -> &str {
        self.0
    }
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// NetlinkDeserializable implementation
impl NetlinkDeserializable for PingPongMessage {
    type Error = DeserializeError;

    fn deserialize(header: &NetlinkHeader, payload: &[u8]) -> Result<Self, Self::Error> {
        match header.message_type {
            PING_MESSAGE => Ok(PingPongMessage::Ping(payload.to_vec())),
            PONG_MESSAGE => Ok(PingPongMessage::Pong(payload.to_vec())),
            _ => Err(DeserializeError(
                "invalid ping-pong message: invalid message type",
            )),
        }
    }
}

// NetlinkSerializable implementation
impl NetlinkSerializable for PingPongMessage {
    fn message_type(&self) -> u16 {
        match self {
            PingPongMessage::Ping(_) => PING_MESSAGE,
            PingPongMessage::Pong(_) => PONG_MESSAGE,
        }
    }

    fn buffer_len(&self) -> usize {
        match self {
            PingPongMessage::Ping(vec) | PingPongMessage::Pong(vec) => vec.len(),
        }
    }

    fn serialize(&self, buffer: &mut [u8]) {
        match self {
            PingPongMessage::Ping(vec) | PingPongMessage::Pong(vec) => {
                buffer.copy_from_slice(&vec[..])
            }
        }
    }
}

// It can be convenient to be able to create a NetlinkMessage directly
// from a PingPongMessage. Since NetlinkMessage<T> already implements
// From<NetlinkPayload<T>>, we just need to implement
// From<NetlinkPayload<PingPongMessage>> for this to work.
impl From<PingPongMessage> for NetlinkPayload<PingPongMessage> {
    fn from(message: PingPongMessage) -> Self {
        NetlinkPayload::InnerMessage(message)
    }
}

fn main() {
    let ping_pong_message = PingPongMessage::Ping(vec![0, 1, 2, 3]);
    let mut packet = NetlinkMessage::from(ping_pong_message);

    // Before serializing the packet, it is very important to call
    // finalize() to ensure the header of the message is consistent
    // with its payload. Otherwise, a panic may occur when calling
    // `serialize()`
    packet.finalize();

    // Prepare a buffer to serialize the packet. Note that we never
    // set explicitely `packet.header.length` above. This was done
    // automatically when we called `finalize()`
    let mut buf = vec![0; packet.header.length as usize];
    // Serialize the packet
    packet.serialize(&mut buf[..]);

    // Deserialize the packet
    let deserialized_packet = NetlinkMessage::<PingPongMessage>::deserialize(&buf)
        .expect("Failed to deserialize message");

    // Normally, the deserialized packet should be exactly the same
    // than the serialized one.
    assert_eq!(deserialized_packet, packet);

    // This should print:
    // NetlinkMessage { header: NetlinkHeader { length: 20, message_type: 18, flags: 0, sequence_number: 0, port_number: 0 }, payload: InnerMessage(Ping([0, 1, 2, 3])) }
    println!("{:?}", packet);
}
