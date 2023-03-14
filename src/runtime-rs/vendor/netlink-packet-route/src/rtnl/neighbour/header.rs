// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
    NeighbourMessageBuffer,
    NEIGHBOUR_HEADER_LEN,
};

/// Neighbour headers have the following structure:
///
/// ```no_rust
/// 0                8                16              24               32
/// +----------------+----------------+----------------+----------------+
/// |     family     |                     padding                      |
/// +----------------+----------------+----------------+----------------+
/// |                             link index                            |
/// +----------------+----------------+----------------+----------------+
/// |              state              |     flags      |     ntype      |
/// +----------------+----------------+----------------+----------------+
/// ```
///
/// `NeighbourHeader` exposes all these fields.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct NeighbourHeader {
    pub family: u8,
    pub ifindex: u32,
    /// Neighbour cache entry state. It should be set to one of the
    /// `NUD_*` constants
    pub state: u16,
    /// Neighbour cache entry flags. It should be set to a combination
    /// of the `NTF_*` constants
    pub flags: u8,
    /// Neighbour cache entry type. It should be set to one of the
    /// `NDA_*` constants.
    pub ntype: u8,
}

impl<T: AsRef<[u8]>> Parseable<NeighbourMessageBuffer<T>> for NeighbourHeader {
    fn parse(buf: &NeighbourMessageBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            family: buf.family(),
            ifindex: buf.ifindex(),
            state: buf.state(),
            flags: buf.flags(),
            ntype: buf.ntype(),
        })
    }
}

impl Emitable for NeighbourHeader {
    fn buffer_len(&self) -> usize {
        NEIGHBOUR_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = NeighbourMessageBuffer::new(buffer);
        packet.set_family(self.family);
        packet.set_ifindex(self.ifindex);
        packet.set_state(self.state);
        packet.set_flags(self.flags);
        packet.set_ntype(self.ntype);
    }
}
