// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Inet6CacheInfo {
    pub max_reasm_len: i32,
    pub tstamp: i32,
    pub reachable_time: i32,
    pub retrans_time: i32,
}

pub const LINK_INET6_CACHE_INFO_LEN: usize = 16;
buffer!(Inet6CacheInfoBuffer(LINK_INET6_CACHE_INFO_LEN) {
    max_reasm_len: (i32, 0..4),
    tstamp: (i32, 4..8),
    reachable_time: (i32, 8..12),
    retrans_time: (i32, 12..16),
});

impl<T: AsRef<[u8]>> Parseable<Inet6CacheInfoBuffer<T>> for Inet6CacheInfo {
    fn parse(buf: &Inet6CacheInfoBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            max_reasm_len: buf.max_reasm_len(),
            tstamp: buf.tstamp(),
            reachable_time: buf.reachable_time(),
            retrans_time: buf.retrans_time(),
        })
    }
}

impl Emitable for Inet6CacheInfo {
    fn buffer_len(&self) -> usize {
        LINK_INET6_CACHE_INFO_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = Inet6CacheInfoBuffer::new(buffer);
        buffer.set_max_reasm_len(self.max_reasm_len);
        buffer.set_tstamp(self.tstamp);
        buffer.set_reachable_time(self.reachable_time);
        buffer.set_retrans_time(self.retrans_time);
    }
}
