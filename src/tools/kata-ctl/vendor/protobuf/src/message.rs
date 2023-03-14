use std::any::Any;
use std::any::TypeId;
use std::fmt;
use std::io::Read;
use std::io::Write;

#[cfg(feature = "bytes")]
use bytes::Bytes;

use crate::clear::Clear;
use crate::coded_input_stream::CodedInputStream;
use crate::coded_input_stream::WithCodedInputStream;
use crate::coded_output_stream::with_coded_output_stream_to_bytes;
use crate::coded_output_stream::CodedOutputStream;
use crate::coded_output_stream::WithCodedOutputStream;
use crate::error::ProtobufError;
use crate::error::ProtobufResult;
use crate::reflect::MessageDescriptor;
use crate::unknown::UnknownFields;

/// Trait implemented for all generated structs for protobuf messages.
///
/// Also, generated messages implement `Clone + Default + PartialEq`
pub trait Message: fmt::Debug + Clear + Any + Send + Sync {
    /// Message descriptor for this message, used for reflection.
    fn descriptor(&self) -> &'static MessageDescriptor;

    /// True iff all required fields are initialized.
    /// Always returns `true` for protobuf 3.
    fn is_initialized(&self) -> bool;

    /// Update this message object with fields read from given stream.
    fn merge_from(&mut self, is: &mut CodedInputStream) -> ProtobufResult<()>;

    /// Parse message from stream.
    fn parse_from(is: &mut CodedInputStream) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut r: Self = Message::new();
        r.merge_from(is)?;
        r.check_initialized()?;
        Ok(r)
    }

    /// Write message to the stream.
    ///
    /// Sizes of this messages and nested messages must be cached
    /// by calling `compute_size` prior to this call.
    fn write_to_with_cached_sizes(&self, os: &mut CodedOutputStream) -> ProtobufResult<()>;

    /// Compute and cache size of this message and all nested messages
    fn compute_size(&self) -> u32;

    /// Get size previously computed by `compute_size`.
    fn get_cached_size(&self) -> u32;

    /// Write the message to the stream.
    ///
    /// Results in error if message is not fully initialized.
    fn write_to(&self, os: &mut CodedOutputStream) -> ProtobufResult<()> {
        self.check_initialized()?;

        // cache sizes
        self.compute_size();
        // TODO: reserve additional
        self.write_to_with_cached_sizes(os)?;

        Ok(())
    }

    /// Write the message to the stream prepending the message with message length
    /// encoded as varint.
    fn write_length_delimited_to(&self, os: &mut CodedOutputStream) -> ProtobufResult<()> {
        let size = self.compute_size();
        os.write_raw_varint32(size)?;
        self.write_to_with_cached_sizes(os)?;

        // TODO: assert we've written same number of bytes as computed

        Ok(())
    }

    /// Write the message to the vec, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_vec(&self, vec: &mut Vec<u8>) -> ProtobufResult<()> {
        let mut os = CodedOutputStream::vec(vec);
        self.write_length_delimited_to(&mut os)?;
        os.flush()?;
        Ok(())
    }

    /// Update this message object with fields read from given stream.
    fn merge_from_bytes(&mut self, bytes: &[u8]) -> ProtobufResult<()> {
        let mut is = CodedInputStream::from_bytes(bytes);
        self.merge_from(&mut is)
    }

    /// Parse message from reader.
    /// Parse stops on EOF or when error encountered.
    fn parse_from_reader(reader: &mut dyn Read) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::new(reader);
        let r = Message::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Parse message from byte array.
    fn parse_from_bytes(bytes: &[u8]) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::from_bytes(bytes);
        let r = Message::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Parse message from `Bytes` object.
    /// Resulting message may share references to the passed bytes object.
    #[cfg(feature = "bytes")]
    fn parse_from_carllerche_bytes(bytes: &Bytes) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::from_carllerche_bytes(bytes);
        let r = Self::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Check if all required fields of this object are initialized.
    fn check_initialized(&self) -> ProtobufResult<()> {
        if !self.is_initialized() {
            Err(ProtobufError::message_not_initialized(
                self.descriptor().name(),
            ))
        } else {
            Ok(())
        }
    }

    /// Write the message to the writer.
    fn write_to_writer(&self, w: &mut dyn Write) -> ProtobufResult<()> {
        w.with_coded_output_stream(|os| self.write_to(os))
    }

    /// Write the message to bytes vec.
    fn write_to_vec(&self, v: &mut Vec<u8>) -> ProtobufResult<()> {
        v.with_coded_output_stream(|os| self.write_to(os))
    }

    /// Write the message to bytes vec.
    fn write_to_bytes(&self) -> ProtobufResult<Vec<u8>> {
        self.check_initialized()?;

        let size = self.compute_size() as usize;
        let mut v = Vec::with_capacity(size);
        // skip zerofill
        unsafe {
            v.set_len(size);
        }
        {
            let mut os = CodedOutputStream::bytes(&mut v);
            self.write_to_with_cached_sizes(&mut os)?;
            os.check_eof();
        }
        Ok(v)
    }

    /// Write the message to the writer, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_writer(&self, w: &mut dyn Write) -> ProtobufResult<()> {
        w.with_coded_output_stream(|os| self.write_length_delimited_to(os))
    }

    /// Write the message to the bytes vec, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_bytes(&self) -> ProtobufResult<Vec<u8>> {
        with_coded_output_stream_to_bytes(|os| self.write_length_delimited_to(os))
    }

    /// Get a reference to unknown fields.
    fn get_unknown_fields<'s>(&'s self) -> &'s UnknownFields;
    /// Get a mutable reference to unknown fields.
    fn mut_unknown_fields<'s>(&'s mut self) -> &'s mut UnknownFields;

    /// Get type id for downcasting.
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    /// View self as `Any`.
    fn as_any(&self) -> &dyn Any;

    /// View self as mutable `Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!()
    }

    /// Convert boxed self to boxed `Any`.
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        panic!()
    }

    // Rust does not allow implementation of trait for trait:
    // impl<M : Message> fmt::Debug for M {
    // ...
    // }

    /// Create an empty message object.
    ///
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let m = MyMessage::new();
    /// # }
    /// ```
    fn new() -> Self
    where
        Self: Sized;

    /// Get message descriptor for message type.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let descriptor = MyMessage::descriptor_static();
    /// assert_eq!("MyMessage", descriptor.name());
    /// # }
    /// ```
    fn descriptor_static() -> &'static MessageDescriptor
    where
        Self: Sized,
    {
        panic!(
            "descriptor_static is not implemented for message, \
             LITE_RUNTIME must be used"
        );
    }

    /// Return a pointer to default immutable message with static lifetime.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let m: &MyMessage = MyMessage::default_instance();
    /// # }
    /// ```
    fn default_instance() -> &'static Self
    where
        Self: Sized;
}

pub fn message_down_cast<'a, M: Message + 'a>(m: &'a dyn Message) -> &'a M {
    m.as_any().downcast_ref::<M>().unwrap()
}

/// Parse message from reader.
/// Parse stops on EOF or when error encountered.
#[deprecated(since = "2.19", note = "Use Message::parse_from_reader instead")]
pub fn parse_from_reader<M: Message>(reader: &mut dyn Read) -> ProtobufResult<M> {
    M::parse_from_reader(reader)
}

/// Parse message from byte array.
#[deprecated(since = "2.19", note = "Use Message::parse_from_bytes instead")]
pub fn parse_from_bytes<M: Message>(bytes: &[u8]) -> ProtobufResult<M> {
    M::parse_from_bytes(bytes)
}

/// Parse message from `Bytes` object.
/// Resulting message may share references to the passed bytes object.
#[cfg(feature = "bytes")]
#[deprecated(
    since = "2.19",
    note = "Use Message::parse_from_carllerche_bytes instead"
)]
pub fn parse_from_carllerche_bytes<M: Message>(bytes: &Bytes) -> ProtobufResult<M> {
    M::parse_from_carllerche_bytes(bytes)
}

/// Parse length-delimited message from stream.
///
/// Read varint length first, and read messages of that length then.
///
/// This function is deprecated and will be removed in the next major release.
#[deprecated]
pub fn parse_length_delimited_from<M: Message>(is: &mut CodedInputStream) -> ProtobufResult<M> {
    is.read_message::<M>()
}

/// Parse length-delimited message from `Read`.
///
/// This function is deprecated and will be removed in the next major release.
#[deprecated]
pub fn parse_length_delimited_from_reader<M: Message>(r: &mut dyn Read) -> ProtobufResult<M> {
    // TODO: wrong: we may read length first, and then read exact number of bytes needed
    r.with_coded_input_stream(|is| is.read_message::<M>())
}

/// Parse length-delimited message from bytes.
///
/// This function is deprecated and will be removed in the next major release.
#[deprecated]
pub fn parse_length_delimited_from_bytes<M: Message>(bytes: &[u8]) -> ProtobufResult<M> {
    bytes.with_coded_input_stream(|is| is.read_message::<M>())
}
