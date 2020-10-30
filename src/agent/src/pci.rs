// Copyright Red Hat.
//
// SPDX-License-Identifier: Apache-2.0
//
use std::convert::TryInto;
use std::fmt;
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

#[cfg(test)]
mod tests {
    use crate::pci::Slot;
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
}
