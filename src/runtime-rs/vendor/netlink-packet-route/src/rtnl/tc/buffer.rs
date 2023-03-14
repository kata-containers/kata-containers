// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const TC_HEADER_LEN: usize = 20;

buffer!(TcMessageBuffer(TC_HEADER_LEN) {
    family: (u8, 0),
    pad1: (u8, 1),
    pad2: (u16, 2..4),
    index: (i32, 4..8),
    handle: (u32, 8..12),
    parent: (u32, 12..16),
    info: (u32, 16..TC_HEADER_LEN),
    payload: (slice, TC_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> TcMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
