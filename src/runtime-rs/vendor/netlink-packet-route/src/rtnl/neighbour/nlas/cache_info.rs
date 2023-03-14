// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CacheInfo {
    pub confirmed: u32,
    pub used: u32,
    pub updated: u32,
    pub refcnt: u32,
}

pub const NEIGHBOUR_CACHE_INFO_LEN: usize = 16;

buffer!(CacheInfoBuffer(NEIGHBOUR_CACHE_INFO_LEN) {
    confirmed: (u32, 0..4),
    used: (u32, 4..8),
    updated: (u32, 8..12),
    refcnt: (u32, 12..16),
});

impl<T: AsRef<[u8]>> Parseable<CacheInfoBuffer<T>> for CacheInfo {
    fn parse(buf: &CacheInfoBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            confirmed: buf.confirmed(),
            used: buf.used(),
            updated: buf.updated(),
            refcnt: buf.refcnt(),
        })
    }
}

impl Emitable for CacheInfo {
    fn buffer_len(&self) -> usize {
        NEIGHBOUR_CACHE_INFO_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = CacheInfoBuffer::new(buffer);
        buffer.set_confirmed(self.confirmed);
        buffer.set_used(self.used);
        buffer.set_updated(self.updated);
        buffer.set_refcnt(self.refcnt);
    }
}
