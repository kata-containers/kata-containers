// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

use super::buffer::{NeighbourTableMessageBuffer, NEIGHBOUR_TABLE_HEADER_LEN};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NeighbourTableHeader {
    pub family: u8,
}

impl<T: AsRef<[u8]>> Parseable<NeighbourTableMessageBuffer<T>> for NeighbourTableHeader {
    fn parse(buf: &NeighbourTableMessageBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            family: buf.family(),
        })
    }
}

impl Emitable for NeighbourTableHeader {
    fn buffer_len(&self) -> usize {
        NEIGHBOUR_TABLE_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = NeighbourTableMessageBuffer::new(buffer);
        packet.set_family(self.family);
    }
}
