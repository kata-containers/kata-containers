// Copyright (C) 2024 Alibaba Cloud. All rights reserved.
//
// Copyright (C) 2025 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use std::cmp::Ordering;
use std::fmt;

use crate::{Error, Result};

const PCI_MAX_DEV_ID: u8 = 0x1f;
const PCI_MAX_FUNC_ID: u8 = 0x7;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct PciAddress {
    /// Bus number, in the range [0, 0xff].
    bus: u8,
    /// Device id, in the range [0x0, 0x1f].
    dev: u8,
    /// Function id, in the range [0x0, 0x7].
    func: u8,
}

impl PartialOrd for PciAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PciAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare in the order of bus -> dev -> func.
        self.bus
            .cmp(&other.bus)
            .then_with(|| self.dev.cmp(&other.dev))
            .then_with(|| self.func.cmp(&other.func))
    }
}

impl PciAddress {
    /// Create a new PCI address from bus and device/function id.
    ///
    /// * `bus`: PCI bus number, in the range \[0x0, 0xff\].
    /// * `dev`: PCI device id, in the range \[0x0, 0x1f\].
    /// * `func`: PCI function id, in the range \[0x0, 0x7\].
    pub fn new(bus: u8, dev: u8, func: u8) -> Result<Self> {
        if dev > PCI_MAX_DEV_ID || func > PCI_MAX_FUNC_ID {
            return Err(Error::InvalidParameter);
        }

        Ok(PciAddress { bus, dev, func })
    }

    /// Get PCI device id on the PCI bus, which is in [0x0, 0x1f]
    pub fn dev_id(&self) -> u8 {
        self.dev
    }

    /// Get PCI device function id, which is in [0x0, 0x7].
    pub fn func_id(&self) -> u8 {
        self.func
    }

    /// Get PCI device bus number, which is in [0x0, 0xff].
    pub fn bus_id(&self) -> u8 {
        self.bus
    }
}

impl fmt::Debug for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PciAddress: {:02x}:{:02x}.{:02x}",
            self.bus, self.dev, self.func
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_address() {
        // test invlaid device id
        assert_eq!(PciAddress::new(0, 32, 0), Err(Error::InvalidParameter));

        // test invalid function id
        assert_eq!(PciAddress::new(0, 0, 8), Err(Error::InvalidParameter));

        // test pci address
        let (bus, dev, func) = (3, 5, 4);
        let address = PciAddress::new(bus, dev, func).unwrap();
        assert_eq!(address.bus_id(), bus);
        assert_eq!(address.dev_id(), dev);
        assert_eq!(address.func_id(), func);
    }
}
