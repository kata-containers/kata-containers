// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

/// Byte/Packet throughput statistics
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StatsBasic {
    /// number of seen bytes
    pub bytes: u64,
    /// number of seen packets
    pub packets: u32,
}

pub const STATS_BASIC_LEN: usize = 12;

buffer!(StatsBasicBuffer(STATS_BASIC_LEN) {
    bytes: (u64, 0..8),
    packets: (u32, 8..12),
});

impl<T: AsRef<[u8]>> Parseable<StatsBasicBuffer<T>> for StatsBasic {
    fn parse(buf: &StatsBasicBuffer<T>) -> Result<Self, DecodeError> {
        Ok(StatsBasic {
            bytes: buf.bytes(),
            packets: buf.packets(),
        })
    }
}

impl Emitable for StatsBasic {
    fn buffer_len(&self) -> usize {
        STATS_BASIC_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = StatsBasicBuffer::new(buffer);
        buffer.set_bytes(self.bytes);
        buffer.set_packets(self.packets);
    }
}
