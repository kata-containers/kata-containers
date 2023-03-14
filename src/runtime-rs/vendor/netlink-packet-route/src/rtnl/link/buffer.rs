// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const LINK_HEADER_LEN: usize = 16;

buffer!(LinkMessageBuffer(LINK_HEADER_LEN) {
    interface_family: (u8, 0),
    reserved_1: (u8, 1),
    link_layer_type: (u16, 2..4),
    link_index: (u32, 4..8),
    flags: (u32, 8..12),
    change_mask: (u32, 12..LINK_HEADER_LEN),
    payload: (slice, LINK_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> LinkMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
