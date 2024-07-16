// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use super::super::common::{CgroupHierarchy, Properties};

use super::transformer::Transformer;

use anyhow::{bail, Result};
use bit_vec::BitVec;
use oci::{LinuxCpu, LinuxResources};
use oci_spec::runtime as oci;
use std::convert::{TryFrom, TryInto};
use zbus::zvariant::Value;

const BASIC_SYSTEMD_VERSION: &str = "244";

pub struct CpuSet {}

impl Transformer for CpuSet {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        _: &CgroupHierarchy,
        systemd_version: &str,
    ) -> Result<()> {
        if let Some(cpuset_resources) = &r.cpu() {
            Self::apply(cpuset_resources, properties, systemd_version)?;
        }

        Ok(())
    }
}

// v1 & v2:
// cpuset.cpus <-> AllowedCPUs (v244)
// cpuset.mems <-> AllowedMemoryNodes (v244)
impl CpuSet {
    fn apply(
        cpuset_resources: &LinuxCpu,
        properties: &mut Properties,
        systemd_version: &str,
    ) -> Result<()> {
        if systemd_version < BASIC_SYSTEMD_VERSION {
            return Ok(());
        }

        if let Some(cpus) = cpuset_resources.cpus().as_ref() {
            let cpus_vec: BitMask = cpus.as_str().try_into()?;
            properties.push(("AllowedCPUs", Value::Array(cpus_vec.0.into())));
        }

        if let Some(mems) = cpuset_resources.mems().as_ref() {
            let mems_vec: BitMask = mems.as_str().try_into()?;
            properties.push(("AllowedMemoryNodes", Value::Array(mems_vec.0.into())));
        }

        Ok(())
    }
}

struct BitMask(Vec<u8>);

impl TryFrom<&str> for BitMask {
    type Error = anyhow::Error;

    fn try_from(bitmask_str: &str) -> Result<Self, Self::Error> {
        let mut bitmask_vec = BitVec::from_elem(8, false);
        let bitmask_str_vec: Vec<&str> = bitmask_str.split(',').collect();
        for bitmask in bitmask_str_vec.iter() {
            let range: Vec<&str> = bitmask.split('-').collect();
            match range.len() {
                1 => {
                    let idx: usize = range[0].parse()?;
                    while idx >= bitmask_vec.len() {
                        bitmask_vec.grow(8, false);
                    }
                    bitmask_vec.set(adjust_index(idx), true);
                }
                2 => {
                    let left_index = range[0].parse()?;
                    let right_index = range[1].parse()?;
                    while right_index >= bitmask_vec.len() {
                        bitmask_vec.grow(8, false);
                    }
                    for idx in left_index..=right_index {
                        bitmask_vec.set(adjust_index(idx), true);
                    }
                }
                _ => bail!("invalid bitmask str {}", bitmask_str),
            }
        }
        let mut result_vec = bitmask_vec.to_bytes();
        result_vec.reverse();

        Ok(BitMask(result_vec))
    }
}

#[inline(always)]
fn adjust_index(idx: usize) -> usize {
    idx / 8 * 8 + 7 - idx % 8
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use crate::cgroups::systemd::subsystem::cpuset::BitMask;

    #[test]
    fn test_bitmask_conversion() {
        let cpus_vec: BitMask = "2-4".try_into().unwrap();
        assert_eq!(vec![0b11100 as u8], cpus_vec.0);

        let cpus_vec: BitMask = "1,7".try_into().unwrap();
        assert_eq!(vec![0b10000010 as u8], cpus_vec.0);

        let cpus_vec: BitMask = "0,2-3,7".try_into().unwrap();
        assert_eq!(vec![0b10001101 as u8], cpus_vec.0);
    }
}
