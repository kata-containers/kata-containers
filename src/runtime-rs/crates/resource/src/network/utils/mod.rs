// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub(crate) mod address;
pub(crate) mod link;

use anyhow::{anyhow, Result};
use rand::rngs::OsRng;
use rand::RngCore;

pub(crate) fn parse_mac(s: &str) -> Option<hypervisor::Address> {
    let v: Vec<_> = s.split(':').collect();
    if v.len() != 6 {
        return None;
    }
    let mut bytes = [0u8; 6];
    for i in 0..6 {
        bytes[i] = u8::from_str_radix(v[i], 16).ok()?;
    }

    Some(hypervisor::Address(bytes))
}

pub(crate) fn get_mac_addr(b: &[u8]) -> Result<String> {
    if b.len() != 6 {
        Err(anyhow!("invalid mac address {:?}", b))
    } else {
        Ok(format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        ))
    }
}

/// Generate a private mac address.
/// The range of private mac addressess is
/// x2-xx-xx-xx-xx-xx, x6-xx-xx-xx-xx-xx, xA-xx-xx-xx-xx-xx, xE-xx-xx-xx-xx-xx.
pub(crate) fn generate_private_mac_addr() -> String {
    let mut addr: [u8; 6] = [0, 0, 0, 0, 0, 0];
    OsRng.fill_bytes(&mut addr);
    addr[0] = (addr[0] | 2) & 0xfe;
    // This is a safty unwrap since the len of addr is 6
    get_mac_addr(&addr).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mac_addr() {
        // length is not 6
        let fail_slice = vec![1, 2, 3];
        assert!(get_mac_addr(&fail_slice).is_err());

        let expected_slice = vec![10, 11, 128, 3, 4, 5];
        let expected_mac = String::from("0a:0b:80:03:04:05");
        let res = get_mac_addr(&expected_slice);
        assert!(res.is_ok());
        assert_eq!(expected_mac, res.unwrap());
    }

    #[test]
    fn test_parse_mac() {
        // length is not 6
        let fail = "1:2:3";
        assert!(parse_mac(fail).is_none());

        let v = [10, 11, 128, 3, 4, 5];
        let expected_addr = hypervisor::Address(v);
        let addr = parse_mac("0a:0b:80:03:04:05");
        assert!(addr.is_some());
        assert_eq!(expected_addr.0, addr.unwrap().0);
    }

    #[test]
    fn test_generate_private_mac_addr() {
        let addr1 = generate_private_mac_addr();
        let addr2 = generate_private_mac_addr();
        assert_ne!(addr1, addr2);
        let ch1 = addr1.chars().nth(1).unwrap();
        let is_private = ch1 == '2' || ch1 == '6' || ch1 == 'a' || ch1 == 'e';
        assert!(is_private)
    }
}
