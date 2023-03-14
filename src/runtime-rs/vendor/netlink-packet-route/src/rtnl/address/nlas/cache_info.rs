// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct CacheInfo {
    pub ifa_preferred: i32,
    pub ifa_valid: i32,
    pub cstamp: i32,
    pub tstamp: i32,
}

pub const ADDRESSS_CACHE_INFO_LEN: usize = 16;
buffer!(CacheInfoBuffer(ADDRESSS_CACHE_INFO_LEN) {
    ifa_preferred: (i32, 0..4),
    ifa_valid: (i32, 4..8),
    cstamp: (i32, 8..12),
    tstamp: (i32, 12..16),
});

impl<T: AsRef<[u8]>> Parseable<CacheInfoBuffer<T>> for CacheInfo {
    fn parse(buf: &CacheInfoBuffer<T>) -> Result<Self, DecodeError> {
        Ok(CacheInfo {
            ifa_preferred: buf.ifa_preferred(),
            ifa_valid: buf.ifa_valid(),
            cstamp: buf.cstamp(),
            tstamp: buf.tstamp(),
        })
    }
}

impl Emitable for CacheInfo {
    fn buffer_len(&self) -> usize {
        ADDRESSS_CACHE_INFO_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = CacheInfoBuffer::new(buffer);
        buffer.set_ifa_preferred(self.ifa_preferred);
        buffer.set_ifa_valid(self.ifa_valid);
        buffer.set_cstamp(self.cstamp);
        buffer.set_tstamp(self.tstamp);
    }
}
