// SPDX-License-Identifier: MIT

use crate::NetlinkHeader;
use std::error::Error;

/// A `NetlinkDeserializable` type can be deserialized from a buffer
pub trait NetlinkDeserializable: Sized {
    type Error: Error + Send + Sync + 'static;

    /// Deserialize the given buffer into `Self`.
    fn deserialize(header: &NetlinkHeader, payload: &[u8]) -> Result<Self, Self::Error>;
}

pub trait NetlinkSerializable {
    fn message_type(&self) -> u16;

    /// Return the length of the serialized data.
    ///
    /// Most netlink messages are encoded following a
    /// [TLV](https://en.wikipedia.org/wiki/Type-length-value) scheme
    /// and this library takes advantage of this by pre-allocating
    /// buffers of the appropriate size when serializing messages,
    /// which is why `buffer_len` is needed.
    fn buffer_len(&self) -> usize;

    /// Serialize this types and write the serialized data into the given buffer.
    /// `buffer`'s length is exactly `InnerMessage::buffer_len()`.
    /// It means that if `InnerMessage::buffer_len()` is buggy and does not return the appropriate length,
    /// bad things can happen:
    ///
    /// - if `buffer_len()` returns a value _smaller than the actual data_, `emit()` may panics
    /// - if `buffer_len()` returns a value _bigger than the actual data_, the buffer will contain garbage
    ///
    /// # Panic
    ///
    /// This method panics if the buffer is not big enough.
    fn serialize(&self, buffer: &mut [u8]);
}
