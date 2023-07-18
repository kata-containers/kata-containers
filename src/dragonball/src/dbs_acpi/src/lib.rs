// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
pub mod rsdp;
pub mod sdt;

fn generate_checksum(data: &[u8]) -> u8 {
    (255 - data.iter().fold(0u8, |acc, x| acc.wrapping_add(*x))).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_generate_checksum() {
        let mut buf = [0x00; 8];
        let sum = generate_checksum(&buf);
        assert_eq!(sum, 0);
        buf[0] = 0xff;
        let sum = generate_checksum(&buf);
        assert_eq!(sum, 1);
        buf[0] = 0xaa;
        buf[1] = 0xcc;
        buf[4] = generate_checksum(&buf);
        let sum = buf.iter().fold(0u8, |s, v| s.wrapping_add(*v));
        assert_eq!(sum, 0);
    }
}
