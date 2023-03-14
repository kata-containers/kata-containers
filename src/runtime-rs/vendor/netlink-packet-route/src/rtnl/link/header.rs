// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
    LinkMessageBuffer,
    LINK_HEADER_LEN,
};

/// High level representation of `RTM_GETLINK`, `RTM_SETLINK`, `RTM_NEWLINK` and `RTM_DELLINK`
/// messages headers.
///
/// These headers have the following structure:
///
/// ```no_rust
/// 0                8                16              24               32
/// +----------------+----------------+----------------+----------------+
/// |interface family|    reserved    |         link layer type         |
/// +----------------+----------------+----------------+----------------+
/// |                             link index                            |
/// +----------------+----------------+----------------+----------------+
/// |                               flags                               |
/// +----------------+----------------+----------------+----------------+
/// |                            change mask                            |
/// +----------------+----------------+----------------+----------------+
/// ```
///
/// `LinkHeader` exposes all these fields except for the "reserved" one.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct LinkHeader {
    /// Address family: one of the `AF_*` constants.
    pub interface_family: u8,
    /// Link index.
    pub index: u32,
    /// Link type. It should be set to one of the `ARPHRD_*`
    /// constants. The most common value is `ARPHRD_ETHER` for
    /// Ethernet.
    pub link_layer_type: u16,
    /// State of the link, described by a combinations of `IFF_*`
    /// constants, for instance `IFF_UP | IFF_LOWER_UP`.
    pub flags: u32,
    /// Change mask for the `flags` field. Reserved, it should be set
    /// to `0xffff_ffff`.
    pub change_mask: u32,
}

impl Emitable for LinkHeader {
    fn buffer_len(&self) -> usize {
        LINK_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = LinkMessageBuffer::new(buffer);
        packet.set_interface_family(self.interface_family);
        packet.set_link_index(self.index);
        packet.set_change_mask(self.change_mask);
        packet.set_link_layer_type(self.link_layer_type);
        packet.set_flags(self.flags);
    }
}

impl<T: AsRef<[u8]>> Parseable<LinkMessageBuffer<T>> for LinkHeader {
    fn parse(buf: &LinkMessageBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            interface_family: buf.interface_family(),
            link_layer_type: buf.link_layer_type(),
            index: buf.link_index(),
            change_mask: buf.change_mask(),
            flags: buf.flags(),
        })
    }
}
