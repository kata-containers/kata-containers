// Copyright (c) IBM Corp. 2021
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;
use std::str::FromStr;

use anyhow::anyhow;

// CCW bus ID follow the format <xx>.<d>.<xxxx> [1, p. 11], where
//   - <xx> is the channel subsystem ID, which is always 0 from the guest side, but different from
//     the host side, e.g. 0xfe for virtio-*-ccw [1, p. 435],
//   - <d> is the subchannel set ID, which ranges from 0-3 [2], and
//   - <xxxx> is the device number (0000-ffff; leading zeroes can be omitted,
//      e.g. 3 instead of 0003).
// [1] https://www.ibm.com/docs/en/linuxonibm/pdf/lku4dd04.pdf
// [2] https://qemu.readthedocs.io/en/master/system/s390x/css.html

// Maximum subchannel set ID
const SUBCHANNEL_SET_MAX: u8 = 3;

// CCW device. From the guest side, the first field is always 0 and can therefore be omitted.
#[derive(Copy, Clone, Debug)]
pub struct Device {
    subchannel_set_id: u8,
    device_number: u16,
}

impl Device {
    pub fn new(subchannel_set_id: u8, device_number: u16) -> anyhow::Result<Self> {
        if subchannel_set_id > SUBCHANNEL_SET_MAX {
            return Err(anyhow!(
                "Subchannel set ID {:?} should be in range [0..{}]",
                subchannel_set_id,
                SUBCHANNEL_SET_MAX
            ));
        }

        Ok(Device {
            subchannel_set_id,
            device_number,
        })
    }
}

impl FromStr for Device {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let split: Vec<&str> = s.split('.').collect();
        if split.len() != 3 {
            return Err(anyhow!(
                "Wrong bus format. It needs to be in the form 0.<d>.<xxxx>, got {:?}",
                s
            ));
        }

        if split[0] != "0" {
            return Err(anyhow!(
                "Wrong bus format. First digit needs to be 0, but is {:?}",
                split[0]
            ));
        }

        let subchannel_set_id = match split[1].parse::<u8>() {
            Ok(id) => id,
            Err(_) => {
                return Err(anyhow!(
                    "Wrong bus format. Second digit needs to be 0-3, but is {:?}",
                    split[1]
                ))
            }
        };

        let device_number = match u16::from_str_radix(split[2], 16) {
            Ok(id) => id,
            Err(_) => {
                return Err(anyhow!(
                    "Wrong bus format. Third digit needs to be 0-ffff, but is {:?}",
                    split[2]
                ))
            }
        };

        Device::new(subchannel_set_id, device_number)
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "0.{}.{:04x}", self.subchannel_set_id, self.device_number)
    }
}

#[cfg(test)]
mod tests {
    use crate::ccw::Device;
    use std::str::FromStr;

    #[test]
    fn test_new_device() {
        // Valid devices
        let device = Device::new(0, 0).unwrap();
        assert_eq!(format!("{}", device), "0.0.0000");

        let device = Device::new(3, 0xffff).unwrap();
        assert_eq!(format!("{}", device), "0.3.ffff");

        // Invalid device
        let device = Device::new(4, 0);
        assert!(device.is_err());
    }

    #[test]
    fn test_device_from_str() {
        // Valid devices
        let device = Device::from_str("0.0.0").unwrap();
        assert_eq!(format!("{}", device), "0.0.0000");

        let device = Device::from_str("0.0.0000").unwrap();
        assert_eq!(format!("{}", device), "0.0.0000");

        let device = Device::from_str("0.3.ffff").unwrap();
        assert_eq!(format!("{}", device), "0.3.ffff");

        // Invalid devices
        let device = Device::from_str("0.0");
        assert!(device.is_err());

        let device = Device::from_str("1.0.0");
        assert!(device.is_err());

        let device = Device::from_str("0.not_a_subchannel_set_id.0");
        assert!(device.is_err());

        let device = Device::from_str("0.0.not_a_device_number");
        assert!(device.is_err());
    }
}
