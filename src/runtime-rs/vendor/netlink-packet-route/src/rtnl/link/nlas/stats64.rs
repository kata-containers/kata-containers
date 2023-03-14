// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const LINK_STATS64_LEN: usize = 192;
buffer!(Stats64Buffer(LINK_STATS64_LEN) {
    rx_packets: (u64, 0..8),
    tx_packets: (u64, 8..16),
    rx_bytes: (u64, 16..24),
    tx_bytes: (u64, 24..32),
    rx_errors: (u64, 32..40),
    tx_errors: (u64, 40..48),
    rx_dropped: (u64, 48..56),
    tx_dropped: (u64, 56..64),
    multicast: (u64, 64..72),
    collisions: (u64, 72..80),
    rx_length_errors: (u64, 80..88),
    rx_over_errors: (u64, 88..96),
    rx_crc_errors: (u64, 96..104),
    rx_frame_errors: (u64, 104..112),
    rx_fifo_errors: (u64, 112..120),
    rx_missed_errors: (u64, 120..128),
    tx_aborted_errors: (u64, 128..136),
    tx_carrier_errors: (u64, 136..144),
    tx_fifo_errors: (u64, 144..152),
    tx_heartbeat_errors: (u64, 152..160),
    tx_window_errors: (u64, 160..168),
    rx_compressed: (u64, 168..176),
    tx_compressed: (u64, 176..184),
    rx_nohandler: (u64, 184..192),
});

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Stats64 {
    /// total packets received
    pub rx_packets: u64,
    /// total packets transmitted
    pub tx_packets: u64,
    /// total bytes received
    pub rx_bytes: u64,
    /// total bytes transmitted
    pub tx_bytes: u64,
    /// bad packets received
    pub rx_errors: u64,
    /// packet transmit problems
    pub tx_errors: u64,
    /// no space in linux buffers
    pub rx_dropped: u64,
    /// no space available in linux
    pub tx_dropped: u64,
    /// multicast packets received
    pub multicast: u64,
    pub collisions: u64,

    // detailed rx_errors
    pub rx_length_errors: u64,
    /// receiver ring buff overflow
    pub rx_over_errors: u64,
    /// received packets with crc error
    pub rx_crc_errors: u64,
    /// received frame alignment errors
    pub rx_frame_errors: u64,
    /// recv'r fifo overrun
    pub rx_fifo_errors: u64,
    /// receiver missed packet
    pub rx_missed_errors: u64,

    // detailed tx_errors
    pub tx_aborted_errors: u64,
    pub tx_carrier_errors: u64,
    pub tx_fifo_errors: u64,
    pub tx_heartbeat_errors: u64,
    pub tx_window_errors: u64,

    // for cslip etc
    pub rx_compressed: u64,
    pub tx_compressed: u64,

    /// dropped, no handler found
    pub rx_nohandler: u64,
}

impl<T: AsRef<[u8]>> Parseable<Stats64Buffer<T>> for Stats64 {
    fn parse(buf: &Stats64Buffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            rx_packets: buf.rx_packets(),
            tx_packets: buf.tx_packets(),
            rx_bytes: buf.rx_bytes(),
            tx_bytes: buf.tx_bytes(),
            rx_errors: buf.rx_errors(),
            tx_errors: buf.tx_errors(),
            rx_dropped: buf.rx_dropped(),
            tx_dropped: buf.tx_dropped(),
            multicast: buf.multicast(),
            collisions: buf.collisions(),
            rx_length_errors: buf.rx_length_errors(),
            rx_over_errors: buf.rx_over_errors(),
            rx_crc_errors: buf.rx_crc_errors(),
            rx_frame_errors: buf.rx_frame_errors(),
            rx_fifo_errors: buf.rx_fifo_errors(),
            rx_missed_errors: buf.rx_missed_errors(),
            tx_aborted_errors: buf.tx_aborted_errors(),
            tx_carrier_errors: buf.tx_carrier_errors(),
            tx_fifo_errors: buf.tx_fifo_errors(),
            tx_heartbeat_errors: buf.tx_heartbeat_errors(),
            tx_window_errors: buf.tx_window_errors(),
            rx_compressed: buf.rx_compressed(),
            tx_compressed: buf.tx_compressed(),
            rx_nohandler: buf.rx_nohandler(),
        })
    }
}

impl Emitable for Stats64 {
    fn buffer_len(&self) -> usize {
        LINK_STATS64_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = Stats64Buffer::new(buffer);
        buffer.set_rx_packets(self.rx_packets);
        buffer.set_tx_packets(self.tx_packets);
        buffer.set_rx_bytes(self.rx_bytes);
        buffer.set_tx_bytes(self.tx_bytes);
        buffer.set_rx_errors(self.rx_errors);
        buffer.set_tx_errors(self.tx_errors);
        buffer.set_rx_dropped(self.rx_dropped);
        buffer.set_tx_dropped(self.tx_dropped);
        buffer.set_multicast(self.multicast);
        buffer.set_collisions(self.collisions);
        buffer.set_rx_length_errors(self.rx_length_errors);
        buffer.set_rx_over_errors(self.rx_over_errors);
        buffer.set_rx_crc_errors(self.rx_crc_errors);
        buffer.set_rx_frame_errors(self.rx_frame_errors);
        buffer.set_rx_fifo_errors(self.rx_fifo_errors);
        buffer.set_rx_missed_errors(self.rx_missed_errors);
        buffer.set_tx_aborted_errors(self.tx_aborted_errors);
        buffer.set_tx_carrier_errors(self.tx_carrier_errors);
        buffer.set_tx_fifo_errors(self.tx_fifo_errors);
        buffer.set_tx_heartbeat_errors(self.tx_heartbeat_errors);
        buffer.set_tx_window_errors(self.tx_window_errors);
        buffer.set_rx_compressed(self.rx_compressed);
        buffer.set_tx_compressed(self.tx_compressed);
        buffer.set_rx_nohandler(self.rx_nohandler);
    }
}
