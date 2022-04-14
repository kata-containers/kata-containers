// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub(crate) mod address;
pub(crate) mod link;
pub(crate) mod netns;

use anyhow::{anyhow, Result};

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
        return Err(anyhow!("invalid mac address {:?}", b));
    } else {
        Ok(format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        ))
    }
}
