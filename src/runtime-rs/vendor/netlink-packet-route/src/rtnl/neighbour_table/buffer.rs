// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const NEIGHBOUR_TABLE_HEADER_LEN: usize = 4;

buffer!(NeighbourTableMessageBuffer(NEIGHBOUR_TABLE_HEADER_LEN) {
    family: (u8, 0),
    payload: (slice, NEIGHBOUR_TABLE_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> NeighbourTableMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
