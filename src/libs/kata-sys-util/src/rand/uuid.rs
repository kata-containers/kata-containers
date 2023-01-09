// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{convert::From, fmt};

use byteorder::{BigEndian, ByteOrder};
use rand::RngCore;

pub struct UUID([u8; 16]);

impl Default for UUID {
    fn default() -> Self {
        Self::new()
    }
}

impl UUID {
    pub fn new() -> Self {
        let mut b = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut b);
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        Self(b)
    }
}

/// From: convert UUID to string
impl From<&UUID> for String {
    fn from(from: &UUID) -> Self {
        let time_low = BigEndian::read_u32(&from.0[..4]);
        let time_mid = BigEndian::read_u16(&from.0[4..6]);
        let time_hi = BigEndian::read_u16(&from.0[6..8]);
        let clk_seq_hi = from.0[8];
        let clk_seq_low = from.0[9];
        let mut buf = [0u8; 8];
        buf[2..].copy_from_slice(&from.0[10..]);
        let node = BigEndian::read_u64(&buf);

        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:012x}",
            time_low, time_mid, time_hi, clk_seq_hi, clk_seq_low, node
        )
    }
}

impl fmt::Display for UUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid() {
        let uuid1 = UUID::new();
        let s1: String = String::from(&uuid1);

        let uuid2 = UUID::new();
        let s2: String = String::from(&uuid2);

        assert_eq!(s1.len(), s2.len());
        assert_ne!(s1, s2);

        let uuid3 = UUID([0u8, 1u8, 2u8, 3u8, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        let s3 = String::from(&uuid3);
        assert_eq!(&s3, "00010203-0405-0607-0809-0a0b0c0d0e0f");
    }
}
