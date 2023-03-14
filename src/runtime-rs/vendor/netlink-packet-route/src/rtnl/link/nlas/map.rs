// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const LINK_MAP_LEN: usize = 28;
buffer!(MapBuffer(LINK_MAP_LEN) {
    memory_start: (u64, 0..8),
    memory_end: (u64, 8..16),
    base_address: (u64, 16..24),
    irq: (u16, 24..26),
    dma: (u8, 26),
    port: (u8, 27),
});

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Map {
    pub memory_start: u64,
    pub memory_end: u64,
    pub base_address: u64,
    pub irq: u16,
    pub dma: u8,
    pub port: u8,
}

impl<T: AsRef<[u8]>> Parseable<MapBuffer<T>> for Map {
    fn parse(buf: &MapBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            memory_start: buf.memory_start(),
            memory_end: buf.memory_end(),
            base_address: buf.base_address(),
            irq: buf.irq(),
            dma: buf.dma(),
            port: buf.port(),
        })
    }
}

impl Emitable for Map {
    fn buffer_len(&self) -> usize {
        LINK_MAP_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = MapBuffer::new(buffer);
        buffer.set_memory_start(self.memory_start);
        buffer.set_memory_end(self.memory_end);
        buffer.set_base_address(self.base_address);
        buffer.set_irq(self.irq);
        buffer.set_dma(self.dma);
        buffer.set_port(self.port);
    }
}
