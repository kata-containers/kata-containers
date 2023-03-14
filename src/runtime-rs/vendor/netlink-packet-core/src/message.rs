// SPDX-License-Identifier: MIT

use anyhow::Context;
use std::fmt::Debug;

use crate::{
    payload::{NLMSG_DONE, NLMSG_ERROR, NLMSG_NOOP, NLMSG_OVERRUN},
    AckMessage,
    DecodeError,
    Emitable,
    ErrorBuffer,
    ErrorMessage,
    NetlinkBuffer,
    NetlinkDeserializable,
    NetlinkHeader,
    NetlinkPayload,
    NetlinkSerializable,
    Parseable,
};

/// Represent a netlink message.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetlinkMessage<I> {
    /// Message header (this is common to all the netlink protocols)
    pub header: NetlinkHeader,
    /// Inner message, which depends on the netlink protocol being used.
    pub payload: NetlinkPayload<I>,
}

impl<I> NetlinkMessage<I> {
    /// Create a new netlink message from the given header and payload
    pub fn new(header: NetlinkHeader, payload: NetlinkPayload<I>) -> Self {
        NetlinkMessage { header, payload }
    }

    /// Consume this message and return its header and payload
    pub fn into_parts(self) -> (NetlinkHeader, NetlinkPayload<I>) {
        (self.header, self.payload)
    }
}

impl<I> NetlinkMessage<I>
where
    I: NetlinkDeserializable,
{
    /// Parse the given buffer as a netlink message
    pub fn deserialize(buffer: &[u8]) -> Result<Self, DecodeError> {
        let netlink_buffer = NetlinkBuffer::new_checked(&buffer)?;
        <Self as Parseable<NetlinkBuffer<&&[u8]>>>::parse(&netlink_buffer)
    }
}

impl<I> NetlinkMessage<I>
where
    I: NetlinkSerializable,
{
    /// Return the length of this message in bytes
    pub fn buffer_len(&self) -> usize {
        <Self as Emitable>::buffer_len(self)
    }

    /// Serialize this message and write the serialized data into the
    /// given buffer. `buffer` must big large enough for the whole
    /// message to fit, otherwise, this method will panic. To know how
    /// big the serialized message is, call `buffer_len()`.
    ///
    /// # Panic
    ///
    /// This method panics if the buffer is not big enough.
    pub fn serialize(&self, buffer: &mut [u8]) {
        self.emit(buffer)
    }

    /// Ensure the header (`NetlinkHeader`) is consistent with the payload (`NetlinkPayload`):
    ///
    /// - compute the payload length and set the header's length field
    /// - check the payload type and set the header's message type field accordingly
    ///
    /// If you are not 100% sure the header is correct, this method should be called before calling
    /// [`Emitable::emit()`](trait.Emitable.html#tymethod.emit), as it could panic if the header is
    /// inconsistent with the rest of the message.
    pub fn finalize(&mut self) {
        self.header.length = self.buffer_len() as u32;
        self.header.message_type = self.payload.message_type();
    }
}

impl<'buffer, B, I> Parseable<NetlinkBuffer<&'buffer B>> for NetlinkMessage<I>
where
    B: AsRef<[u8]> + 'buffer,
    I: NetlinkDeserializable,
{
    fn parse(buf: &NetlinkBuffer<&'buffer B>) -> Result<Self, DecodeError> {
        use self::NetlinkPayload::*;

        let header = <NetlinkHeader as Parseable<NetlinkBuffer<&'buffer B>>>::parse(buf)
            .context("failed to parse netlink header")?;

        let bytes = buf.payload();
        let payload = match header.message_type {
            NLMSG_ERROR => {
                let buf =
                    ErrorBuffer::new_checked(&bytes).context("failed to parse NLMSG_ERROR")?;
                let msg = ErrorMessage::parse(&buf).context("failed to parse NLMSG_ERROR")?;
                if msg.code >= 0 {
                    Ack(msg as AckMessage)
                } else {
                    Error(msg)
                }
            }
            NLMSG_NOOP => Noop,
            NLMSG_DONE => Done,
            NLMSG_OVERRUN => Overrun(bytes.to_vec()),
            message_type => {
                let inner_msg = I::deserialize(&header, bytes).context(format!(
                    "Failed to parse message with type {}",
                    message_type
                ))?;
                InnerMessage(inner_msg)
            }
        };
        Ok(NetlinkMessage { header, payload })
    }
}

impl<I> Emitable for NetlinkMessage<I>
where
    I: NetlinkSerializable,
{
    fn buffer_len(&self) -> usize {
        use self::NetlinkPayload::*;

        let payload_len = match self.payload {
            Noop | Done => 0,
            Overrun(ref bytes) => bytes.len(),
            Error(ref msg) => msg.buffer_len(),
            Ack(ref msg) => msg.buffer_len(),
            InnerMessage(ref msg) => msg.buffer_len(),
        };

        self.header.buffer_len() + payload_len
    }

    fn emit(&self, buffer: &mut [u8]) {
        use self::NetlinkPayload::*;

        self.header.emit(buffer);

        let buffer = &mut buffer[self.header.buffer_len()..self.header.length as usize];
        match self.payload {
            Noop | Done => {}
            Overrun(ref bytes) => buffer.copy_from_slice(bytes),
            Error(ref msg) => msg.emit(buffer),
            Ack(ref msg) => msg.emit(buffer),
            InnerMessage(ref msg) => msg.serialize(buffer),
        }
    }
}

impl<T> From<T> for NetlinkMessage<T>
where
    T: Into<NetlinkPayload<T>>,
{
    fn from(inner_message: T) -> Self {
        NetlinkMessage {
            header: NetlinkHeader::default(),
            payload: inner_message.into(),
        }
    }
}
