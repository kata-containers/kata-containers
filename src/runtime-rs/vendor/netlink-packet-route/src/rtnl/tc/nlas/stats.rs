// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

/// Generic queue statistics
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Stats {
    /// Number of enqueued bytes
    pub bytes: u64,
    /// Number of enqueued packets
    pub packets: u32,
    /// Packets dropped because of lack of resources
    pub drops: u32,
    /// Number of throttle events when this flow goes out of allocated bandwidth
    pub overlimits: u32,
    /// Current flow byte rate
    pub bps: u32,
    /// Current flow packet rate
    pub pps: u32,
    pub qlen: u32,
    pub backlog: u32,
}

pub const STATS_LEN: usize = 36;

buffer!(StatsBuffer(STATS_LEN) {
    bytes: (u64, 0..8),
    packets: (u32, 8..12),
    drops: (u32, 12..16),
    overlimits: (u32, 16..20),
    bps: (u32, 20..24),
    pps: (u32, 24..28),
    qlen: (u32, 28..32),
    backlog: (u32, 32..36),
});

impl<T: AsRef<[u8]>> Parseable<StatsBuffer<T>> for Stats {
    fn parse(buf: &StatsBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            bytes: buf.bytes(),
            packets: buf.packets(),
            drops: buf.drops(),
            overlimits: buf.overlimits(),
            bps: buf.bps(),
            pps: buf.pps(),
            qlen: buf.qlen(),
            backlog: buf.backlog(),
        })
    }
}

impl Emitable for Stats {
    fn buffer_len(&self) -> usize {
        STATS_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = StatsBuffer::new(buffer);
        buffer.set_bytes(self.bytes);
        buffer.set_packets(self.packets);
        buffer.set_drops(self.drops);
        buffer.set_overlimits(self.overlimits);
        buffer.set_bps(self.bps);
        buffer.set_pps(self.pps);
        buffer.set_qlen(self.qlen);
        buffer.set_backlog(self.backlog);
    }
}
