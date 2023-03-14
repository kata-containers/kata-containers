// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const NEIGHBOUR_HEADER_LEN: usize = 12;
buffer!(NeighbourMessageBuffer(NEIGHBOUR_HEADER_LEN) {
    family: (u8, 0),
    ifindex: (u32, 4..8),
    state: (u16, 8..10),
    flags: (u8, 10),
    ntype: (u8, 11),
    payload:(slice, NEIGHBOUR_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> NeighbourMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
