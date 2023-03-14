// SPDX-License-Identifier: MIT

use crate::{buffer::NETLINK_HEADER_LEN, DecodeError, Emitable, NetlinkBuffer, Parseable};

/// A Netlink header representation. A netlink header has the following structure:
///
/// ```no_rust
/// 0                8                16              24               32
/// +----------------+----------------+----------------+----------------+
/// |                 packet length (including header)                  |
/// +----------------+----------------+----------------+----------------+
/// |          message type           |              flags              |
/// +----------------+----------------+----------------+----------------+
/// |                           sequence number                         |
/// +----------------+----------------+----------------+----------------+
/// |                   port number (formerly known as PID)             |
/// +----------------+----------------+----------------+----------------+
/// ```
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
pub struct NetlinkHeader {
    /// Length of the netlink packet, including the header and the payload
    pub length: u32,

    /// NetlinkMessage type. The meaning of this field depends on the netlink protocol family in use.
    pub message_type: u16,

    /// Flags. It should be set to one of the `NLM_F_*` constants.
    pub flags: u16,

    /// Sequence number of the packet
    pub sequence_number: u32,

    /// Port number (usually set to the the process ID)
    pub port_number: u32,
}

impl Emitable for NetlinkHeader {
    fn buffer_len(&self) -> usize {
        NETLINK_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = NetlinkBuffer::new(buffer);
        buffer.set_message_type(self.message_type);
        buffer.set_length(self.length);
        buffer.set_flags(self.flags);
        buffer.set_sequence_number(self.sequence_number);
        buffer.set_port_number(self.port_number);
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NetlinkBuffer<&'a T>> for NetlinkHeader {
    fn parse(buf: &NetlinkBuffer<&'a T>) -> Result<NetlinkHeader, DecodeError> {
        Ok(NetlinkHeader {
            length: buf.length(),
            message_type: buf.message_type(),
            flags: buf.flags(),
            sequence_number: buf.sequence_number(),
            port_number: buf.port_number(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::*;

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

    const RTM_GETLINK: u16 = 18;

    #[test]
    fn repr_parse() {
        let repr =
            NetlinkHeader::parse(&NetlinkBuffer::new_checked(&IP_LINK_SHOW_PKT[..]).unwrap())
                .unwrap();
        assert_eq!(repr.length, 40);
        assert_eq!(repr.message_type, RTM_GETLINK);
        assert_eq!(repr.sequence_number, 1_526_271_540);
        assert_eq!(repr.port_number, 0);
        assert_eq!(repr.flags, NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH);
    }

    #[test]
    fn repr_emit() {
        let repr = NetlinkHeader {
            length: 40,
            message_type: RTM_GETLINK,
            sequence_number: 1_526_271_540,
            flags: NLM_F_ROOT | NLM_F_REQUEST | NLM_F_MATCH,
            port_number: 0,
        };
        assert_eq!(repr.buffer_len(), 16);
        let mut buf = vec![0; 16];
        repr.emit(&mut buf[..]);
        assert_eq!(&buf[..], &IP_LINK_SHOW_PKT[..16]);
    }
}
