// SPDX-License-Identifier: MIT

use crate::{
    nlas::{NlaBuffer, NlasIterator},
    DecodeError,
};

pub const RULE_HEADER_LEN: usize = 12;

buffer!(RuleMessageBuffer(RULE_HEADER_LEN) {
    family: (u8, 0),
    dst_len: (u8, 1),
    src_len: (u8, 2),
    tos: (u8, 3),
    table: (u8, 4),
    reserve_1: (u8, 5),
    reserve_2: (u8, 6),
    action: (u8, 7),
    flags: (u32, 8..RULE_HEADER_LEN),
    payload: (slice, RULE_HEADER_LEN..),
});

impl<'a, T: AsRef<[u8]> + ?Sized> RuleMessageBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(self.payload())
    }
}
