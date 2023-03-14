// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const ADDRESS_HEADER_LEN: usize = 8;

buffer!(AddressMessageBuffer(ADDRESS_HEADER_LEN) {
    family: (u8, 0),
    prefix_len: (u8, 1),
    flags: (u8, 2),
    scope: (u8, 3),
    index: (u32, 4..ADDRESS_HEADER_LEN),
    payload: (slice, ADDRESS_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> AddressMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
