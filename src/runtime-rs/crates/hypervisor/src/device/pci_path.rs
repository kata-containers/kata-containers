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

// The PCI spec reserves 5 bits for slot number (a.k.a. device
// number), giving slots 0..31
const PCI_SLOT_BITS: u32 = 5;
const MAX_PCI_SLOTS: u32 = (1 << PCI_SLOT_BITS) - 1;

// A PciSlot describes where a PCI device sits on a single bus
//
// This encapsulates the PCI slot number a.k.a device number, which is
// limited to a 5 bit value [0x00..0x1f] by the PCI specification
//
// To support multifunction device's, It's needed to extend
// this to include the PCI 3-bit function number as well.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PciSlot(pub u8);

impl PciSlot {
    pub fn new(v: u8) -> PciSlot {
        PciSlot(v)
    }
}

impl std::fmt::Display for PciSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}", self.0)
    }
}

impl TryFrom<&str> for PciSlot {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<PciSlot> {
        if s.is_empty() || s.len() > 2 {
            return Err(anyhow!("string given is invalid."));
        }

        let base = 16;
        let n = u64::from_str_radix(s, base).context(format!(
            "convert string to number with base {:?} failed.",
            base
        ))?;
        if n >> PCI_SLOT_BITS > 0 {
            return Err(anyhow!(
                "number {:?} exceeds MAX:{:?}, failed.",
                n,
                MAX_PCI_SLOTS
            ));
        }

        Ok(PciSlot(n as u8))
    }
}

impl TryFrom<u32> for PciSlot {
    type Error = anyhow::Error;

    fn try_from(v: u32) -> Result<PciSlot> {
        if v > MAX_PCI_SLOTS {
            return Err(anyhow!("value {:?} exceeds MAX: {:?}", v, MAX_PCI_SLOTS));
        }

        Ok(PciSlot(v as u8))
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
                .map(|pci_slot| format!("{:02x}", pci_slot.0))
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
        // min
        let pci_slot_01 = PciSlot::try_from("00");
        assert!(pci_slot_01.is_ok());
        // max
        let pci_slot_02 = PciSlot::try_from("1f");
        assert!(pci_slot_02.is_ok());

        // exceed
        let pci_slot_03 = PciSlot::try_from("20");
        assert!(pci_slot_03.is_err());

        // valid number
        let pci_slot_04 = PciSlot::try_from(1_u32);
        assert!(pci_slot_04.is_ok());
        assert_eq!(pci_slot_04.as_ref().unwrap().0, 1_u8);
        let pci_slot_str = pci_slot_04.as_ref().unwrap().to_string();
        assert_eq!(pci_slot_str, format!("{:02x}", pci_slot_04.unwrap().0));

        // max number
        let pci_slot_05 = PciSlot::try_from(31_u32);
        assert!(pci_slot_05.is_ok());
        assert_eq!(pci_slot_05.unwrap().0, 31_u8);

        // exceed and error
        let pci_slot_06 = PciSlot::try_from(32_u32);
        assert!(pci_slot_06.is_err());
    }

    #[test]
    fn test_pci_patch() {
        let pci_path_0 = PciPath::try_from("01/0a/05");
        assert!(pci_path_0.is_ok());
        let pci_path_unwrap = pci_path_0.unwrap();
        assert_eq!(pci_path_unwrap.slots[0].0, 1);
        assert_eq!(pci_path_unwrap.slots[1].0, 10);
        assert_eq!(pci_path_unwrap.slots[2].0, 5);

        let pci_path_01 = PciPath::new(vec![PciSlot(1), PciSlot(10), PciSlot(5)]);
        assert!(pci_path_01.is_some());
        let pci_path = pci_path_01.unwrap();
        let pci_path_02 = pci_path.to_string();
        assert_eq!(pci_path_02, "01/0a/05".to_string());

        let dev_slot = pci_path.get_device_slot();
        assert!(dev_slot.is_some());
        assert_eq!(dev_slot.unwrap().0, 5);

        let root_slot = pci_path.get_root_slot();
        assert!(root_slot.is_some());
        assert_eq!(root_slot.unwrap().0, 1);
    }
}
