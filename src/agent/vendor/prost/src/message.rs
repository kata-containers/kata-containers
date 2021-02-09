use std::fmt::Debug;
use std::usize;

use ::bytes::{Buf, BufMut, IntoBuf};

use crate::encoding::*;
use crate::DecodeError;
use crate::EncodeError;

/// A Protocol Buffers message.
pub trait Message: Debug + Send + Sync {
    /// Encodes the message to a buffer.
    ///
    /// This method will panic if the buffer has insufficient capacity.
    ///
    /// Meant to be used only by `Message` implementations.
    #[doc(hidden)]
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
        Self: Sized;

    /// Decodes a field from a buffer, and merges it into `self`.
    ///
    /// Meant to be used only by `Message` implementations.
    #[doc(hidden)]
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
        Self: Sized;

    /// Returns the encoded length of the message without a length delimiter.
    fn encoded_len(&self) -> usize;

    /// Encodes the message to a buffer.
    ///
    /// An error will be returned if the buffer does not have sufficient capacity.
    fn encode<B>(&self, buf: &mut B) -> Result<(), EncodeError>
    where
        B: BufMut,
        Self: Sized,
    {
        let required = self.encoded_len();
        let remaining = buf.remaining_mut();
        if required > buf.remaining_mut() {
            return Err(EncodeError::new(required, remaining));
        }

        self.encode_raw(buf);
        Ok(())
    }

    /// Encodes the message with a length-delimiter to a buffer.
    ///
    /// An error will be returned if the buffer does not have sufficient capacity.
    fn encode_length_delimited<B>(&self, buf: &mut B) -> Result<(), EncodeError>
    where
        B: BufMut,
        Self: Sized,
    {
        let len = self.encoded_len();
        let required = len + encoded_len_varint(len as u64);
        let remaining = buf.remaining_mut();
        if required > remaining {
            return Err(EncodeError::new(required, remaining));
        }
        encode_varint(len as u64, buf);
        self.encode_raw(buf);
        Ok(())
    }

    /// Decodes an instance of the message from a buffer.
    ///
    /// The entire buffer will be consumed.
    fn decode<B>(buf: B) -> Result<Self, DecodeError>
    where
        B: IntoBuf,
        Self: Default,
    {
        let mut message = Self::default();
        Self::merge(&mut message, &mut buf.into_buf()).map(|_| message)
    }

    /// Decodes a length-delimited instance of the message from the buffer.
    fn decode_length_delimited<B>(buf: B) -> Result<Self, DecodeError>
    where
        B: IntoBuf,
        Self: Default,
    {
        let mut message = Self::default();
        message.merge_length_delimited(buf)?;
        Ok(message)
    }

    /// Decodes an instance of the message from a buffer, and merges it into `self`.
    ///
    /// The entire buffer will be consumed.
    fn merge<B>(&mut self, buf: B) -> Result<(), DecodeError>
    where
        B: IntoBuf,
        Self: Sized,
    {
        let mut buf = buf.into_buf();
        while buf.has_remaining() {
            self.merge_field(&mut buf)?;
        }
        Ok(())
    }

    /// Decodes a length-delimited instance of the message from buffer, and
    /// merges it into `self`.
    fn merge_length_delimited<B>(&mut self, buf: B) -> Result<(), DecodeError>
    where
        B: IntoBuf,
        Self: Sized,
    {
        message::merge(WireType::LengthDelimited, self, &mut buf.into_buf())
    }

    /// Clears the message, resetting all fields to their default.
    fn clear(&mut self);
}

impl<M> Message for Box<M>
where
    M: Message,
{
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        (**self).encode_raw(buf)
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        (**self).merge_field(buf)
    }
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }
    fn clear(&mut self) {
        (**self).clear()
    }
}
