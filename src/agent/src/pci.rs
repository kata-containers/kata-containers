// Copyright Red Hat.
//
// SPDX-License-Identifier: Apache-2.0
//
use std::convert::TryInto;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use anyhow::anyhow;

// The PCI spec reserves 5 bits for slot number (a.k.a. device
// number), giving slots 0..31
const SLOT_BITS: u8 = 5;
const SLOT_MAX: u8 = (1 << SLOT_BITS) - 1;

// Represents a PCI function's slot number (a.k.a. device number),
// giving its location on a single bus
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Slot(u8);

impl Slot {
    pub fn new<T: TryInto<u8> + fmt::Display + Copy>(v: T) -> anyhow::Result<Self> {
        if let Ok(v8) = v.try_into() {
            if v8 <= SLOT_MAX {
                return Ok(Slot(v8));
            }
        }
        Err(anyhow!(
            "PCI slot {} should be in range [0..{:#x}]",
            v,
            SLOT_MAX
        ))
    }
}

impl FromStr for Slot {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let v = isize::from_str_radix(s, 16)?;
        Slot::new(v)
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:02x}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path(Vec<Slot>);

impl Path {
    pub fn new(slots: Vec<Slot>) -> anyhow::Result<Self> {
        if slots.is_empty() {
            return Err(anyhow!("PCI path must have at least one element"));
        }
        Ok(Path(slots))
    }
}

// Let Path be treated as a slice of Slots
impl Deref for Path {
    type Target = [Slot];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let sslots: Vec<String> = self
            .0
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        write!(f, "{}", sslots.join("/"))
    }
}

impl FromStr for Path {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let rslots: anyhow::Result<Vec<Slot>> = s.split('/').map(Slot::from_str).collect();
        Path::new(rslots?)
    }
}

#[cfg(test)]
mod tests {
    use crate::pci::{Path, Slot};
    use std::str::FromStr;

    #[test]
    fn test_slot() {
        // Valid slots
        let slot = Slot::new(0x00).unwrap();
        assert_eq!(format!("{}", slot), "00");

        let slot = Slot::from_str("00").unwrap();
        assert_eq!(format!("{}", slot), "00");

        let slot = Slot::new(31).unwrap();
        let slot2 = Slot::from_str("1f").unwrap();
        assert_eq!(slot, slot2);

        // Bad slots
        let slot = Slot::new(-1);
        assert!(slot.is_err());

        let slot = Slot::new(32);
        assert!(slot.is_err());

        let slot = Slot::from_str("20");
        assert!(slot.is_err());

        let slot = Slot::from_str("xy");
        assert!(slot.is_err());

        let slot = Slot::from_str("00/");
        assert!(slot.is_err());

        let slot = Slot::from_str("");
        assert!(slot.is_err());
    }

    #[test]
    fn test_path() {
        let slot3 = Slot::new(0x03).unwrap();
        let slot4 = Slot::new(0x04).unwrap();
        let slot5 = Slot::new(0x05).unwrap();

        // Valid paths
        let pcipath = Path::new(vec![slot3]).unwrap();
        assert_eq!(format!("{}", pcipath), "03");
        let pcipath2 = Path::from_str("03").unwrap();
        assert_eq!(pcipath, pcipath2);
        assert_eq!(pcipath.len(), 1);
        assert_eq!(pcipath[0], slot3);

        let pcipath = Path::new(vec![slot3, slot4]).unwrap();
        assert_eq!(format!("{}", pcipath), "03/04");
        let pcipath2 = Path::from_str("03/04").unwrap();
        assert_eq!(pcipath, pcipath2);
        assert_eq!(pcipath.len(), 2);
        assert_eq!(pcipath[0], slot3);
        assert_eq!(pcipath[1], slot4);

        let pcipath = Path::new(vec![slot3, slot4, slot5]).unwrap();
        assert_eq!(format!("{}", pcipath), "03/04/05");
        let pcipath2 = Path::from_str("03/04/05").unwrap();
        assert_eq!(pcipath, pcipath2);
        assert_eq!(pcipath.len(), 3);
        assert_eq!(pcipath[0], slot3);
        assert_eq!(pcipath[1], slot4);
        assert_eq!(pcipath[2], slot5);

        // Bad paths
        assert!(Path::new(vec!()).is_err());
        assert!(Path::from_str("20").is_err());
        assert!(Path::from_str("//").is_err());
        assert!(Path::from_str("xyz").is_err());
    }
}
