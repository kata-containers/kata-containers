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

// The kata-agent decides whether the first element of a PCI path names a root
// bus in pcipath_from_dev_tree_path() (src/agent/src/linux_abi.rs): it must be
// a bus number of at least PXB_PCIE_ROOT_BUS_MIN, and the path must have at
// least MIN_ROOTED_PATH_ELEMENTS elements. Both must match the agent.
const PXB_PCIE_ROOT_BUS_MIN: u8 = 0x20;
const MIN_ROOTED_PATH_ELEMENTS: usize = 3;

fn is_root_bus(root_bus: u8, element_count: usize) -> bool {
    root_bus >= PXB_PCIE_ROOT_BUS_MIN && element_count >= MIN_ROOTED_PATH_ELEMENTS
}

// A PciSlot describes where a PCI function sits on a single bus: a device
// number, limited to [0x00..0x1f] by the PCI specification, and a function
// number, limited to [0..7].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PciSlot {
    device: u8,
    function: u8,
}

impl PciSlot {
    pub fn new(device: u8, function: u8) -> Result<PciSlot> {
        if u32::from(device) > MAX_PCI_SLOTS {
            return Err(anyhow!(
                "PCI device {} exceeds maximum {}",
                device,
                MAX_PCI_SLOTS
            ));
        }
        if u32::from(function) > MAX_PCI_FUNCTIONS {
            return Err(anyhow!(
                "PCI function {} exceeds maximum {}",
                function,
                MAX_PCI_FUNCTIONS
            ));
        }

        Ok(PciSlot { device, function })
    }

    pub fn device(self) -> u8 {
        self.device
    }

    pub fn function(self) -> u8 {
        self.function
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
        let (device, function) = match s.split('.').collect::<Vec<_>>()[..] {
            [device] => (device, "0"),
            [device, function] => (device, function),
            _ => return Err(anyhow!("PCI slot {s:?} should have the format DD or DD.F")),
        };
        if device.is_empty() || device.len() > 2 || function.len() != 1 {
            return Err(anyhow!("PCI slot {s:?} should have the format DD or DD.F"));
        }

        let device = u8::from_str_radix(device, 16)
            .with_context(|| format!("invalid PCI device in slot {s:?}"))?;
        let function = u8::from_str_radix(function, 16)
            .with_context(|| format!("invalid PCI function in slot {s:?}"))?;
        PciSlot::new(device, function)
    }
}

impl TryFrom<u32> for PciSlot {
    type Error = anyhow::Error;

    fn try_from(v: u32) -> Result<PciSlot> {
        if v > MAX_PCI_SLOTS {
            return Err(anyhow!("value {:?} exceeds MAX: {:?}", v, MAX_PCI_SLOTS));
        }

        PciSlot::new(v as u8, 0)
    }
}

// A PciPath describes where a PCI function sits in a PCI hierarchy.
//
// It consists of the slot of each bridge that must be traversed from the root
// bus to reach the device, followed by the slot of the device itself, and of
// the root bus when that is not the default root complex (bus 00).
//
// Its string form is "[bb/]dd[.f]/.../dd[.f]". bb is the root bus in hex and
// is only present for a non-default root complex; each dd[.f] is a slot
// (device dd, function f, with ".f" omitted for function 0). The first slot is
// the "most upstream" bridge on the root bus and the last one is the device.
// A device directly connected to the root bus, as with the lightweight
// hypervisors (dragonball, firecracker, clh), has a single slot.
//
// A leading element is only read as a root bus when is_root_bus() holds, and
// the constructors reject root buses that would not, so that every PciPath
// round-trips through Display and TryFrom<&str>.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PciPath {
    root_bus: Option<u8>,
    slots: Vec<PciSlot>,
}

impl PciPath {
    pub fn new(slots: Vec<PciSlot>) -> Option<PciPath> {
        Self::new_with_root_bus(None, slots)
    }

    pub fn new_with_root_bus(root_bus: Option<u8>, slots: Vec<PciSlot>) -> Option<PciPath> {
        if slots.is_empty() {
            return None;
        }
        if root_bus.is_some_and(|root_bus| !is_root_bus(root_bus, slots.len() + 1)) {
            return None;
        }

        Some(PciPath { root_bus, slots })
    }

    // root_bus returns the root bus of a non-default root complex
    pub fn root_bus(&self) -> Option<u8> {
        self.root_bus
    }

    pub fn slots(&self) -> &[PciSlot] {
        &self.slots
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
        let slots = self
            .slots
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join("/");
        match self.root_bus {
            Some(root_bus) => write!(f, "{root_bus:02x}/{slots}"),
            None => write!(f, "{slots}"),
        }
    }
}

// convert from u32
impl TryFrom<u32> for PciPath {
    type Error = anyhow::Error;

    fn try_from(slot: u32) -> Result<PciPath> {
        PciPath::new(vec![
            PciSlot::try_from(slot).context("pci slot convert failed.")?
        ])
        .ok_or_else(|| anyhow!("PCI path has no slot"))
    }
}

impl PciPath {
    // to_device_numbers is the inverse of TryFrom<u32> for each slot, for
    // interfaces that only carry device numbers, such as the legacy AddSwap
    // RPC. It fails for paths it cannot express: on a non-default root
    // complex, or through a function other than 0.
    pub fn to_device_numbers(&self) -> Result<Vec<u32>> {
        if let Some(root_bus) = self.root_bus {
            return Err(anyhow!(
                "PCI path {} is on root bus {:02x}, not on the default root complex",
                self,
                root_bus
            ));
        }

        self.slots
            .iter()
            .map(|slot| {
                if slot.function() != 0 {
                    return Err(anyhow!(
                        "PCI path {} goes through PCI function {}",
                        self,
                        slot.function()
                    ));
                }
                Ok(u32::from(slot.device()))
            })
            .collect()
    }
}

impl TryFrom<&str> for PciPath {
    type Error = anyhow::Error;

    // method to parse a PciPath from a string
    fn try_from(path: &str) -> Result<PciPath> {
        if path.is_empty() {
            return Err(anyhow!("path given is empty."));
        }

        let mut elements = path.split('/').collect::<Vec<_>>();
        // Anything that is not a root bus in the leading position is parsed as
        // a slot, e.g. the multifunction "08.1/00/00".
        let root_bus = Some(elements[0])
            .filter(|first| first.len() == 2)
            .and_then(|first| u8::from_str_radix(first, 16).ok())
            .filter(|root_bus| is_root_bus(*root_bus, elements.len()));
        if root_bus.is_some() {
            elements.remove(0);
        }

        let slots = elements
            .into_iter()
            .map(|slot| {
                PciSlot::try_from(slot)
                    .with_context(|| format!("invalid PCI slot in path {path:?}"))
            })
            .collect::<Result<Vec<_>>>()?;

        PciPath::new_with_root_bus(root_bus, slots)
            .ok_or_else(|| anyhow!("invalid PCI path {path:?}"))
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
        assert_eq!(PciSlot::new(8, 1).unwrap(), multifunction);
        assert_eq!(multifunction.to_string(), "08.1");

        let maximum = PciSlot::try_from("1f.7").unwrap();
        assert_eq!(maximum.device(), 31);
        assert_eq!(maximum.function(), 7);

        assert!(PciSlot::new(0x20, 0).is_err());
        assert!(PciSlot::new(0x80, 0).is_err());
        assert!(PciSlot::new(0, 8).is_err());
        assert!(PciSlot::try_from("20").is_err());
        assert!(PciSlot::try_from("00.8").is_err());
        assert!(PciSlot::try_from("00.0.0").is_err());
        assert!(PciSlot::try_from("00.").is_err());
        assert!(PciSlot::try_from("000").is_err());
        assert!(PciSlot::try_from("").is_err());
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
        assert_eq!(legacy_path.root_bus(), None);
        assert_eq!(legacy_path.slots()[0].device(), 1);
        assert_eq!(legacy_path.slots()[1].device(), 10);
        assert_eq!(legacy_path.slots()[2].device(), 5);

        let rooted_path = PciPath::try_from("80/00/00").unwrap();
        assert_eq!(rooted_path.to_string(), "80/00/00");
        assert_eq!(rooted_path.root_bus(), Some(0x80));
        assert_eq!(rooted_path.get_root_slot().unwrap().device(), 0);
        assert_eq!(rooted_path.get_device_slot().unwrap().device(), 0);

        let multifunction_path = PciPath::try_from("08.1/00/00").unwrap();
        assert_eq!(multifunction_path.root_bus(), None);
        assert_eq!(multifunction_path.slots().len(), 3);
        assert_eq!(multifunction_path.to_string(), "08.1/00/00");

        let max_slot_path = PciPath::try_from("1f/00/00").unwrap();
        assert_eq!(max_slot_path.root_bus(), None);
        assert_eq!(max_slot_path.slots().len(), 3);

        assert!(PciPath::try_from("80/00").is_err());
        assert!(PciPath::try_from("").is_err());
    }

    #[test]
    fn test_pci_path_root_bus_round_trip() {
        let slot = PciSlot::new(0, 0).unwrap();
        let slots = vec![slot, slot];
        assert!(PciPath::new_with_root_bus(Some(0x1f), slots.clone()).is_none());
        assert!(PciPath::new_with_root_bus(Some(0x80), vec![slot]).is_none());
        assert!(PciPath::new(Vec::new()).is_none());

        for path in [
            PciPath::new_with_root_bus(Some(0x20), slots.clone()).unwrap(),
            PciPath::new_with_root_bus(Some(0x80), slots.clone()).unwrap(),
            PciPath::new(vec![PciSlot::new(0x1f, 0).unwrap(), slot, slot]).unwrap(),
            PciPath::try_from(0x1f_u32).unwrap(),
            PciPath::try_from("08.1/00/00").unwrap(),
        ] {
            assert_eq!(PciPath::try_from(path.to_string().as_str()).unwrap(), path);
        }
    }

    #[test]
    fn test_pci_path_to_device_numbers() {
        let path = PciPath::try_from("08/00").unwrap();
        assert_eq!(path.to_device_numbers().unwrap(), vec![8, 0]);
        assert_eq!(
            PciPath::try_from(3_u32)
                .unwrap()
                .to_device_numbers()
                .unwrap(),
            vec![3]
        );

        let error = PciPath::try_from("08.1/00")
            .unwrap()
            .to_device_numbers()
            .unwrap_err()
            .to_string();
        assert!(error.contains("PCI function 1"));
        assert!(error.contains("08.1/00"));

        let error = PciPath::try_from("80/00/00")
            .unwrap()
            .to_device_numbers()
            .unwrap_err()
            .to_string();
        assert!(error.contains("root bus 80"));
        assert!(error.contains("80/00/00"));
    }
}
