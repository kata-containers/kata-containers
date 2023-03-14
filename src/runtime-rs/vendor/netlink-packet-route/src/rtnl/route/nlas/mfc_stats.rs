// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MfcStats {
    pub packets: u64,
    pub bytes: u64,
    pub wrong_if: u64,
}

pub const MFC_STATS_LEN: usize = 24;

buffer!(MfcStatsBuffer(MFC_STATS_LEN) {
    packets: (u64, 0..8),
    bytes: (u64, 8..16),
    wrong_if: (u64, 16..24),
});

impl<T: AsRef<[u8]>> Parseable<MfcStatsBuffer<T>> for MfcStats {
    fn parse(buf: &MfcStatsBuffer<T>) -> Result<MfcStats, DecodeError> {
        Ok(MfcStats {
            packets: buf.packets(),
            bytes: buf.bytes(),
            wrong_if: buf.wrong_if(),
        })
    }
}

impl Emitable for MfcStats {
    fn buffer_len(&self) -> usize {
        MFC_STATS_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = MfcStatsBuffer::new(buffer);
        buffer.set_packets(self.packets);
        buffer.set_bytes(self.bytes);
        buffer.set_wrong_if(self.wrong_if);
    }
}
