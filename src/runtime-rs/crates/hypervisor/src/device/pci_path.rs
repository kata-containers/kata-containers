// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use anyhow::{anyhow, Context, Result};

// Tips:
// The Re-write `PciSlot` and `PciPath` with rust that it origins from `pcipath.go`:
//

// The PCI spec reserves 5 bits for the device number and 3 bits for the
// function number.
const PCI_SLOT_BITS: u32 = 5;
const PCI_FUNCTION_BITS: u32 = 3;
const MAX_PCI_SLOTS: u32 = (1 << PCI_SLOT_BITS) - 1;
const MAX_PCI_FUNCTIONS: u32 = (1 << PCI_FUNCTION_BITS) - 1;

// A PciSlot describes where a PCI device sits on a single bus
//
// This encapsulates the PCI slot number a.k.a device number, which is
// limited to a 5 bit value [0x00..0x1f] by the PCI specification
//
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PciSlot {
    device: u8,
    function: u8,
}

impl PciSlot {
    pub fn new(device: u8) -> PciSlot {
        PciSlot {
            device,
            function: 0,
        }
    }

    pub fn new_with_function(device: u8, function: u8) -> Result<PciSlot> {
        if device as u32 > MAX_PCI_SLOTS {
            return Err(anyhow!(
                "PCI device {} exceeds maximum {}",
                device,
                MAX_PCI_SLOTS
            ));
        }
        if function as u32 > MAX_PCI_FUNCTIONS {
            return Err(anyhow!(
                "PCI function {} exceeds maximum {}",
                function,
                MAX_PCI_FUNCTIONS
            ));
        }

        Ok(PciSlot { device, function })
    }

    pub fn from_devfn(devfn: u8) -> PciSlot {
        PciSlot {
            device: devfn >> PCI_FUNCTION_BITS,
            function: devfn & MAX_PCI_FUNCTIONS as u8,
        }
    }

    pub fn device(self) -> u8 {
        self.device
    }

    pub fn function(self) -> u8 {
        self.function
    }

    pub fn devfn(self) -> u8 {
        (self.device << PCI_FUNCTION_BITS) | self.function
    }
}

impl std::fmt::Display for PciSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.function == 0 {
            write!(f, "{:02x}", self.device)
        } else {
            write!(f, "{:02x}.{:x}", self.device, self.function)
        }
    }
}

impl TryFrom<&str> for PciSlot {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<PciSlot> {
        let mut parts = s.split('.');
        let device = parts
            .next()
            .filter(|device| !device.is_empty() && device.len() <= 2)
            .ok_or_else(|| anyhow!("PCI device is invalid: {s}"))?;
        let function = parts.next();
        if parts.next().is_some() {
            return Err(anyhow!("PCI slot/function is invalid: {s}"));
        }

        let base = 16;
        let device = u8::from_str_radix(device, base).context(format!(
            "convert string to number with base {base:?} failed."
        ))?;
        let function = function
            .map(|function| {
                if function.is_empty() || function.len() > 1 {
                    return Err(anyhow!("PCI function is invalid: {s}"));
                }
                u8::from_str_radix(function, base)
                    .with_context(|| format!("convert PCI function {function:?} failed"))
            })
            .transpose()?
            .unwrap_or_default();

        PciSlot::new_with_function(device, function)
    }
}

impl TryFrom<u32> for PciSlot {
    type Error = anyhow::Error;

    fn try_from(v: u32) -> Result<PciSlot> {
        if v > MAX_PCI_SLOTS {
            return Err(anyhow!("value {:?} exceeds MAX: {:?}", v, MAX_PCI_SLOTS));
        }

        Ok(PciSlot::new(v as u8))
    }
}

// A PciPath describes where a PCI sits in a PCI hierarchy.
//
// Consists of a list of PCI slots, giving the slot of each bridge
// that must be traversed from the PCI root to reach the device,
// followed by the slot of the device itself.
//
// When formatted into a string is written as "xx/.../yy/zz". Here,
// zz is the slot of the device on its PCI bridge, yy is the slot of
// the bridge on its parent bridge and so forth until xx is the slot
// of the "most upstream" bridge on the root bus.
//
// If a device is directly connected to the root bus, which used in
// lightweight hypervisors, such as dragonball/firecracker/clh, and
// its PciPath.slots will contains only one PciSlot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PciPath {
    // list of PCI slots
    pub slots: Vec<PciSlot>,
}

impl PciPath {
    pub fn new(slots: Vec<PciSlot>) -> Option<PciPath> {
        if slots.is_empty() {
            return None;
        }

        Some(PciPath { slots })
    }

    // device_slot to get the slot of the device on its PCI bridge
    pub fn get_device_slot(&self) -> Option<PciSlot> {
        self.slots.last().cloned()
    }

    // root_slot to get the slot of the "most upstream" bridge on the root bus
    pub fn get_root_slot(&self) -> Option<PciSlot> {
        self.slots.first().cloned()
    }
}

impl std::fmt::Display for PciPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.slots
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("/")
        )
    }
}

// convert from u32
impl TryFrom<u32> for PciPath {
    type Error = anyhow::Error;

    fn try_from(slot: u32) -> Result<PciPath> {
        Ok(PciPath {
            slots: vec![PciSlot::try_from(slot).context("pci slot convert failed.")?],
        })
    }
}

impl TryFrom<&str> for PciPath {
    type Error = anyhow::Error;

    // method to parse a PciPath from a string
    fn try_from(path: &str) -> Result<PciPath> {
        if path.is_empty() {
            return Err(anyhow!("path given is empty."));
        }

        let mut pci_slots: Vec<PciSlot> = Vec::new();
        let slots: Vec<&str> = path.split('/').collect();
        for slot in slots {
            match PciSlot::try_from(slot) {
                Ok(s) => pci_slots.push(s),
                Err(e) => return Err(anyhow!("slot is invalid with: {:?}", e)),
            }
        }

        Ok(PciPath { slots: pci_slots })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_slot() {
        let function_zero = PciSlot::try_from("01.0").unwrap();
        assert_eq!(function_zero.device(), 1);
        assert_eq!(function_zero.function(), 0);
        assert_eq!(function_zero.to_string(), "01");

        let multifunction = PciSlot::try_from("08.1").unwrap();
        assert_eq!(multifunction.device(), 8);
        assert_eq!(multifunction.function(), 1);
        assert_eq!(multifunction.devfn(), 0x41);
        assert_eq!(PciSlot::from_devfn(0x41), multifunction);
        assert_eq!(multifunction.to_string(), "08.1");

        let maximum = PciSlot::try_from("1f.7").unwrap();
        assert_eq!(maximum.device(), 31);
        assert_eq!(maximum.function(), 7);
        assert_eq!(maximum.devfn(), u8::MAX);

        assert!(PciSlot::try_from("20").is_err());
        assert!(PciSlot::try_from("00.8").is_err());
        assert!(PciSlot::try_from("00.0.0").is_err());
        assert!(PciSlot::try_from(32_u32).is_err());
    }

    #[test]
    fn test_pci_path() {
        let pci_path = PciPath::try_from("08.1/00.0").unwrap();
        assert_eq!(pci_path.to_string(), "08.1/00");
        assert_eq!(pci_path.get_root_slot().unwrap().device(), 8);
        assert_eq!(pci_path.get_root_slot().unwrap().function(), 1);
        assert_eq!(pci_path.get_device_slot().unwrap().device(), 0);

        let legacy_path = PciPath::try_from("01/0a/05").unwrap();
        assert_eq!(legacy_path.to_string(), "01/0a/05");
        assert_eq!(legacy_path.slots[0].device(), 1);
        assert_eq!(legacy_path.slots[1].device(), 10);
        assert_eq!(legacy_path.slots[2].device(), 5);
    }
}
