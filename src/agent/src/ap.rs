// Copyright (c) IBM Corp. 2023
//
// SPDX-License-Identifier: Apache-2.0
//
use std::fmt;
use std::str::FromStr;

use anyhow::{anyhow, Context};

// IBM Adjunct Processor (AP) is used for cryptographic operations
// by IBM Crypto Express hardware security modules on IBM zSystem & LinuxONE (s390x).
// In Linux, virtual cryptographic devices are called AP queues.
// The name of an AP queue respects a format <xx>.<xxxx> in hexadecimal notation [1, p.467]:
//   - <xx> is an adapter ID
//   - <xxxx> is an adapter domain ID
// [1] https://www.ibm.com/docs/en/linuxonibm/pdf/lku5dd05.pdf

#[derive(Debug)]
pub struct Address {
    pub adapter_id: u8,
    pub adapter_domain: u16,
}

impl Address {
    pub fn new(adapter_id: u8, adapter_domain: u16) -> Address {
        Address {
            adapter_id,
            adapter_domain,
        }
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let split: Vec<&str> = s.split('.').collect();
        if split.len() != 2 {
            return Err(anyhow!(
                "Wrong AP bus format. It needs to be in the form <xx>.<xxxx> (e.g. 0a.003f), got {:?}",
                s
            ));
        }

        let adapter_id = u8::from_str_radix(split[0], 16).context(format!(
            "Wrong AP bus format. AP ID needs to be in the form <xx> (e.g. 0a), got {:?}",
            split[0]
        ))?;
        let adapter_domain = u16::from_str_radix(split[1], 16).context(format!(
            "Wrong AP bus format. AP domain needs to be in the form <xxxx> (e.g. 003f), got {:?}",
            split[1]
        ))?;

        Ok(Address::new(adapter_id, adapter_domain))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:02x}.{:04x}", self.adapter_id, self.adapter_domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        let device = Address::from_str("a.1").unwrap();
        assert_eq!(format!("{}", device), "0a.0001");

        assert!(Address::from_str("").is_err());
        assert!(Address::from_str(".").is_err());
        assert!(Address::from_str("0.0.0").is_err());
        assert!(Address::from_str("0g.0000").is_err());
        assert!(Address::from_str("0a.10000").is_err());
    }
}
