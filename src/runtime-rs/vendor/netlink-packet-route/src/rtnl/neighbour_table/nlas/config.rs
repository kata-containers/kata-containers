// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Config {
    pub key_len: u16,
    pub entry_size: u16,
    pub entries: u32,
    pub last_flush: u32,
    pub last_rand: u32,
    pub hash_rand: u32,
    pub hash_mask: u32,
    pub hash_chain_gc: u32,
    pub proxy_qlen: u32,
}

pub const CONFIG_LEN: usize = 32;

buffer!(ConfigBuffer(CONFIG_LEN) {
    key_len: (u16, 0..2),
    entry_size: (u16, 2..4),
    entries: (u32, 4..8),
    last_flush: (u32, 8..12),
    last_rand: (u32, 12..16),
    hash_rand: (u32, 16..20),
    hash_mask: (u32, 20..24),
    hash_chain_gc: (u32, 24..28),
    proxy_qlen: (u32, 28..32),
});

impl<T: AsRef<[u8]>> Parseable<ConfigBuffer<T>> for Config {
    fn parse(buf: &ConfigBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            key_len: buf.key_len(),
            entry_size: buf.entry_size(),
            entries: buf.entries(),
            last_flush: buf.last_flush(),
            last_rand: buf.last_rand(),
            hash_rand: buf.hash_rand(),
            hash_mask: buf.hash_mask(),
            hash_chain_gc: buf.hash_chain_gc(),
            proxy_qlen: buf.proxy_qlen(),
        })
    }
}

impl Emitable for Config {
    fn buffer_len(&self) -> usize {
        CONFIG_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = ConfigBuffer::new(buffer);
        buffer.set_key_len(self.key_len);
        buffer.set_entry_size(self.entry_size);
        buffer.set_entries(self.entries);
        buffer.set_last_flush(self.last_flush);
        buffer.set_last_rand(self.last_rand);
        buffer.set_hash_rand(self.hash_rand);
        buffer.set_hash_mask(self.hash_mask);
        buffer.set_hash_chain_gc(self.hash_chain_gc);
        buffer.set_proxy_qlen(self.proxy_qlen);
    }
}
