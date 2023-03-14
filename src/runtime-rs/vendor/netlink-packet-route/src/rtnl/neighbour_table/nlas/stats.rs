// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Stats {
    pub allocs: u64,
    pub destroys: u64,
    pub hash_grows: u64,
    pub res_failed: u64,
    pub lookups: u64,
    pub hits: u64,
    pub multicast_probes_received: u64,
    pub unicast_probes_received: u64,
    pub periodic_gc_runs: u64,
    pub forced_gc_runs: u64,
}

pub const STATS_LEN: usize = 80;
buffer!(StatsBuffer(STATS_LEN) {
    allocs: (u64, 0..8),
    destroys: (u64, 8..16),
    hash_grows: (u64, 16..24),
    res_failed: (u64, 24..32),
    lookups: (u64, 32..40),
    hits: (u64, 40..48),
    multicast_probes_received: (u64, 48..56),
    unicast_probes_received: (u64, 56..64),
    periodic_gc_runs: (u64, 64..72),
    forced_gc_runs: (u64, 72..80),
});

impl<T: AsRef<[u8]>> Parseable<StatsBuffer<T>> for Stats {
    fn parse(buf: &StatsBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            allocs: buf.allocs(),
            destroys: buf.destroys(),
            hash_grows: buf.hash_grows(),
            res_failed: buf.res_failed(),
            lookups: buf.lookups(),
            hits: buf.hits(),
            multicast_probes_received: buf.multicast_probes_received(),
            unicast_probes_received: buf.unicast_probes_received(),
            periodic_gc_runs: buf.periodic_gc_runs(),
            forced_gc_runs: buf.forced_gc_runs(),
        })
    }
}

impl Emitable for Stats {
    fn buffer_len(&self) -> usize {
        STATS_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = StatsBuffer::new(buffer);
        buffer.set_allocs(self.allocs);
        buffer.set_destroys(self.destroys);
        buffer.set_hash_grows(self.hash_grows);
        buffer.set_res_failed(self.res_failed);
        buffer.set_lookups(self.lookups);
        buffer.set_hits(self.hits);
        buffer.set_multicast_probes_received(self.multicast_probes_received);
        buffer.set_unicast_probes_received(self.unicast_probes_received);
        buffer.set_periodic_gc_runs(self.periodic_gc_runs);
        buffer.set_forced_gc_runs(self.forced_gc_runs);
    }
}
