// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use super::super::common::{CgroupHierarchy, Properties};

use super::transformer::Transformer;

use anyhow::{bail, Result};
use oci::{LinuxMemory, LinuxResources};
use oci_spec::runtime as oci;
use zbus::zvariant::Value;

pub struct Memory {}

impl Transformer for Memory {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        cgroup_hierarchy: &CgroupHierarchy,
        _: &str,
    ) -> Result<()> {
        if let Some(memory_resources) = &r.memory() {
            match cgroup_hierarchy {
                CgroupHierarchy::Legacy => Self::legacy_apply(memory_resources, properties)?,
                CgroupHierarchy::Unified => Self::unified_apply(memory_resources, properties)?,
            }
        }

        Ok(())
    }
}

impl Memory {
    // v1:
    // memory.limit <-> MemoryLimit
    fn legacy_apply(memory_resources: &LinuxMemory, properties: &mut Properties) -> Result<()> {
        if let Some(limit) = memory_resources.limit() {
            let limit = match limit {
                1..=i64::MAX => limit as u64,
                0 => u64::MAX,
                _ => bail!("invalid memory.limit"),
            };
            properties.push(("MemoryLimit", Value::U64(limit)));
        }

        Ok(())
    }

    // v2:
    // memory.low <-> MemoryLow
    // memory.max <-> MemoryMax
    // memory.swap & memory.limit <-> MemorySwapMax
    fn unified_apply(memory_resources: &LinuxMemory, properties: &mut Properties) -> Result<()> {
        if let Some(limit) = memory_resources.limit() {
            let limit = match limit {
                1..=i64::MAX => limit as u64,
                0 => u64::MAX,
                _ => bail!("invalid memory.limit: {}", limit),
            };
            properties.push(("MemoryMax", Value::U64(limit)));
        }

        if let Some(reservation) = memory_resources.reservation() {
            let reservation = match reservation {
                1..=i64::MAX => reservation as u64,
                0 => u64::MAX,
                _ => bail!("invalid memory.reservation: {}", reservation),
            };
            properties.push(("MemoryLow", Value::U64(reservation)));
        }

        let swap = match memory_resources.swap() {
            Some(0) => u64::MAX,
            Some(1..=i64::MAX) => match memory_resources.limit() {
                Some(1..=i64::MAX) => {
                    (memory_resources.limit().unwrap() - memory_resources.swap().unwrap()) as u64
                }
                _ => bail!("invalid memory.limit when memory.swap specified"),
            },
            None => u64::MAX,
            _ => bail!("invalid memory.swap"),
        };

        properties.push(("MemorySwapMax", Value::U64(swap)));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Memory;
    use super::Properties;
    use super::Value;
    use oci_spec::runtime as oci;

    #[test]
    fn test_unified_memory() {
        let memory_resources = oci::LinuxMemoryBuilder::default()
            .limit(736870912)
            .reservation(536870912)
            .swap(536870912)
            .kernel(0)
            .kernel_tcp(0)
            .swappiness(0u64)
            .disable_oom_killer(false)
            .build()
            .unwrap();

        let mut properties: Properties = vec![];

        assert_eq!(
            true,
            Memory::unified_apply(&memory_resources, &mut properties).is_ok()
        );

        assert_eq!(Value::U64(200000000), properties[2].1);
    }
}
