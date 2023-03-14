// SPDX-License-Identifier: MIT

use byteorder::{ByteOrder, NativeEndian};

use crate::{DecodeError, Field, Rest};

const LENGTH: Field = 0..4;
const MESSAGE_TYPE: Field = 4..6;
const FLAGS: Field = 6..8;
const SEQUENCE_NUMBER: Field = 8..12;
const PORT_NUMBER: Field = 12..16;
const PAYLOAD: Rest = 16..;

/// Length of a Netlink packet header
pub const NETLINK_HEADER_LEN: usize = PAYLOAD.start;

// Prevent some doctest snippers to be formatted, since we cannot add
// the attribute directly in the doctest
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Clone)]
/// A raw Netlink buffer that provides getters and setter for the various header fields, and to
/// retrieve the payloads.
///
/// # Example: reading a packet
///
/// ```rust
/// use netlink_packet_core::{NetlinkBuffer, NLM_F_MATCH, NLM_F_REQUEST, NLM_F_ROOT};
///
/// const RTM_GETLINK: u16 = 18;
///
/// fn main() {
///     // Artificially create an array of bytes that represents a netlink packet.
///     // Normally, we would read it from a socket.
///     let buffer = vec![
///         0x28, 0x00, 0x00, 0x00, // length = 40
///         0x12, 0x00, // message type = 18 (RTM_GETLINK)
///         0x01, 0x03, // flags = Request + Specify Tree Root + Return All Matching
///         0x34, 0x0e, 0xf9, 0x5a, // sequence number = 1526271540
///         0x00, 0x00, 0x00, 0x00, // port id = 0
///         // payload
///         0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///         0x08, 0x00, 0x1d, 0x00, 0x01, 0x00, 0x00, 0x00];
///
///     // Wrap the storage into a NetlinkBuffer
///     let packet = NetlinkBuffer::new_checked(&buffer[..]).unwrap();
///
///     // Check that the different accessor return the expected values
///     assert_eq!(packet.length(), 40);
///     assert_eq!(packet.message_type(), RTM_GETLINK);
///     assert_eq!(packet.sequence_number(), 1526271540);
///     assert_eq!(packet.port_number(), 0);
///     assert_eq!(packet.payload_length(), 24);
///     assert_eq!(packet.payload(), &buffer[16..]);
///     assert_eq!(
///         Into::<u16>::into(packet.flags()),
///         NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH
///     );
/// }
/// ```
///
/// # Example: writing a packet
///
/// ```rust
/// use netlink_packet_core::{NetlinkBuffer, NLM_F_MATCH, NLM_F_REQUEST, NLM_F_ROOT};
///
/// const RTM_GETLINK: u16 = 18;
///
/// fn main() {
///     // The packet we want to write.
///     let expected_buffer = vec![
///         0x28, 0x00, 0x00, 0x00, // length = 40
///         0x12, 0x00, // message type = 18 (RTM_GETLINK)
///         0x01, 0x03, // flags = Request + Specify Tree Root + Return All Matching
///         0x34, 0x0e, 0xf9, 0x5a, // sequence number = 1526271540
///         0x00, 0x00, 0x00, 0x00, // port id = 0
///         // payload
///         0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
///         0x08, 0x00, 0x1d, 0x00, 0x01, 0x00, 0x00, 0x00];
///
///     // Create a storage that is big enough for our packet
///     let mut buf = vec![0; 40];
///     // the extra scope is to restrict the scope of the borrow
///     {
///         // Create a NetlinkBuffer.
///         let mut packet = NetlinkBuffer::new(&mut buf);
///         // Set the various fields
///         packet.set_length(40);
///         packet.set_message_type(RTM_GETLINK);
///         packet.set_sequence_number(1526271540);
///         packet.set_port_number(0);
///         packet.set_flags(From::from(NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH));
///         // we kind of cheat here to keep the example short
///         packet.payload_mut().copy_from_slice(&expected_buffer[16..]);
///     }
///     // Check that the storage contains the expected values
///     assert_eq!(&buf[..], &expected_buffer[..]);
/// }
/// ```
///
/// Note that in this second example we don't call
/// [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked) because the length field is
/// initialized to 0, so `new_checked()` would return an error.
pub struct NetlinkBuffer<T> {
    pub buffer: T,
}

// Prevent some doc strings to be formatted, since we cannot add the
// attribute directly in the doctest
#[rustfmt::skip]
impl<T: AsRef<[u8]>> NetlinkBuffer<T> {
    /// Create a new `NetlinkBuffer` that uses the given buffer as storage. Note that when calling
    /// this method no check is performed, so trying to access fields may panic. If you're not sure
    /// the given buffer contains a valid netlink packet, use
    /// [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked) instead.
    pub fn new(buffer: T) -> NetlinkBuffer<T> {
        NetlinkBuffer { buffer }
    }

    // Prevent some doc strings to be formatted, since we cannot add
    // the attribute directly in the doctest
    #[rustfmt::skip]
    /// Check the length of the given buffer and make sure it's big enough so that trying to access
    /// packet fields won't panic. If the buffer is big enough, create a new `NewlinkBuffer` that
    /// uses this buffer as storage.
    ///
    /// # Example
    ///
    /// With a buffer that does not even contain a full header:
    ///
    /// ```rust
    /// use netlink_packet_core::NetlinkBuffer;
    /// static BYTES: [u8; 4] = [0x28, 0x00, 0x00, 0x00];
    /// assert!(NetlinkBuffer::new_checked(&BYTES[..]).is_err());
    /// ```
    ///
    /// Here is a slightly more tricky error, where technically, the buffer is big enough to
    /// contains a valid packet. Here, accessing the packet header fields would not panic but
    /// accessing the payload would, so `new_checked` also checks the length field in the packet
    /// header:
    ///
    /// ```rust
    /// use netlink_packet_core::NetlinkBuffer;
    /// // The buffer is 24 bytes long. It contains a valid header but a truncated payload
    /// static BYTES: [u8; 24] = [
    ///     // The length field says the buffer is 40 bytes long
    ///     0x28, 0x00, 0x00, 0x00,
    ///     0x12, 0x00, // message type
    ///     0x01, 0x03, // flags
    ///     0x34, 0x0e, 0xf9, 0x5a, // sequence number
    ///     0x00, 0x00, 0x00, 0x00, // port id
    ///     // payload
    ///     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    /// assert!(NetlinkBuffer::new_checked(&BYTES[..]).is_err());
    /// ```
    pub fn new_checked(buffer: T) -> Result<NetlinkBuffer<T>, DecodeError> {
        let packet = Self::new(buffer);
        packet.check_buffer_length()?;
        Ok(packet)
    }

    fn check_buffer_length(&self) -> Result<(), DecodeError> {
        let len = self.buffer.as_ref().len();
        if len < PORT_NUMBER.end {
            Err(format!(
                "invalid netlink buffer: length is {} but netlink packets are at least {} bytes",
                len, PORT_NUMBER.end
            )
            .into())
        } else if len < self.length() as usize {
            Err(format!(
                "invalid netlink buffer: length field says {} the buffer is {} bytes long",
                self.length(),
                len
            )
            .into())
        } else if (self.length() as usize) < PORT_NUMBER.end {
            Err(format!(
                "invalid netlink buffer: length field says {} but netlink packets are at least {} bytes",
                self.length(),
                len
            ).into())
        } else {
            Ok(())
        }
    }

    /// Return the payload length.
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small or if the `length` field in the header is
    /// set to a value that exceeds the storage length (see
    /// [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn payload_length(&self) -> usize {
        let total_length = self.length() as usize;
        let payload_offset = PAYLOAD.start;
        // This may panic!
        total_length - payload_offset
    }

    /// Consume the packet, returning the underlying buffer.
    pub fn into_inner(self) -> T {
        self.buffer
    }

    /// Return the `length` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn length(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[LENGTH])
    }

    /// Return the `type` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn message_type(&self) -> u16 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u16(&data[MESSAGE_TYPE])
    }

    /// Return the `flags` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn flags(&self) -> u16 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u16(&data[FLAGS])
    }

    /// Return the `sequence_number` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn sequence_number(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[SEQUENCE_NUMBER])
    }

    /// Return the `port_number` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn port_number(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[PORT_NUMBER])
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> NetlinkBuffer<T> {
    /// Set the packet header `length` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn set_length(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        NativeEndian::write_u32(&mut data[LENGTH], value)
    }

    /// Set the packet header `message_type` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn set_message_type(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        NativeEndian::write_u16(&mut data[MESSAGE_TYPE], value)
    }

    /// Set the packet header `flags` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn set_flags(&mut self, value: u16) {
        let data = self.buffer.as_mut();
        NativeEndian::write_u16(&mut data[FLAGS], value)
    }

    /// Set the packet header `sequence_number` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn set_sequence_number(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        NativeEndian::write_u32(&mut data[SEQUENCE_NUMBER], value)
    }

    /// Set the packet header `port_number` field
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small (see [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn set_port_number(&mut self, value: u32) {
        let data = self.buffer.as_mut();
        NativeEndian::write_u32(&mut data[PORT_NUMBER], value)
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> NetlinkBuffer<&'a T> {
    /// Return a pointer to the packet payload.
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small or if the `length` field in the header is
    /// set to a value that exceeds the storage length (see
    /// [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn payload(&self) -> &'a [u8] {
        let range = PAYLOAD.start..self.length() as usize;
        let data = self.buffer.as_ref();
        &data[range]
    }
}

impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> NetlinkBuffer<&'a mut T> {
    /// Return a mutable pointer to the payload.
    ///
    /// # Panic
    ///
    /// This panic is the underlying storage is too small or if the `length` field in the header is
    /// set to a value that exceeds the storage length (see
    /// [`new_checked()`](struct.NetlinkBuffer.html#method.new_checked))
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let range = PAYLOAD.start..self.length() as usize;
        let data = self.buffer.as_mut();
        &mut data[range]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        constants::{NLM_F_MATCH, NLM_F_REQUEST, NLM_F_ROOT},
        NetlinkBuffer,
    };

    const RTM_GETLINK: u16 = 18;

    // a packet captured with tcpdump that was sent when running `ip link show`
    #[rustfmt::skip]
    static IP_LINK_SHOW_PKT: [u8; 40] = [
        0x28, 0x00, 0x00, 0x00, // length = 40
        0x12, 0x00, // message type = 18 (RTM_GETLINK)
        0x01, 0x03, // flags = Request + Specify Tree Root + Return All Matching
        0x34, 0x0e, 0xf9, 0x5a, // sequence number = 1526271540
        0x00, 0x00, 0x00, 0x00, // port id = 0
        // payload
        0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x08, 0x00, 0x1d, 0x00, 0x01, 0x00, 0x00, 0x00];

    #[test]
    fn packet_read() {
        let packet = NetlinkBuffer::new(&IP_LINK_SHOW_PKT[..]);
        assert_eq!(packet.length(), 40);
        assert_eq!(packet.message_type(), RTM_GETLINK);
        assert_eq!(packet.sequence_number(), 1526271540);
        assert_eq!(packet.port_number(), 0);
        let flags = packet.flags();
        assert!(flags & NLM_F_ROOT == NLM_F_ROOT);
        assert!(flags & NLM_F_REQUEST == NLM_F_REQUEST);
        assert!(flags & NLM_F_MATCH == NLM_F_MATCH);
        assert_eq!(flags, NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH);
        assert_eq!(packet.payload_length(), 24);
        assert_eq!(packet.payload(), &IP_LINK_SHOW_PKT[16..]);
    }

    #[test]
    fn packet_build() {
        let mut buf = vec![0; 40];
        {
            let mut packet = NetlinkBuffer::new(&mut buf);
            packet.set_length(40);
            packet.set_message_type(RTM_GETLINK);
            packet.set_sequence_number(1526271540);
            packet.set_port_number(0);
            packet.set_flags((NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH));
            packet
                .payload_mut()
                .copy_from_slice(&IP_LINK_SHOW_PKT[16..]);
        }
        assert_eq!(&buf[..], &IP_LINK_SHOW_PKT[..]);
    }
}
