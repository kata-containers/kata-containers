// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use super::super::common::{CgroupHierarchy, Properties};

use super::transformer::Transformer;

use anyhow::Result;
use oci::{LinuxPids, LinuxResources};
use oci_spec::runtime as oci;
use zbus::zvariant::Value;

pub struct Pids {}

impl Transformer for Pids {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        _: &CgroupHierarchy,
        _: &str,
    ) -> Result<()> {
        if let Some(pids_resources) = &r.pids() {
            Self::apply(pids_resources, properties)?;
        }

        Ok(())
    }
}

// pids.limit <-> TasksMax
impl Pids {
    fn apply(pids_resources: &LinuxPids, properties: &mut Properties) -> Result<()> {
        let limit = if pids_resources.limit() > 0 {
            pids_resources.limit() as u64
        } else {
            u64::MAX
        };

        properties.push(("TasksMax", Value::U64(limit)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Pids;
    use super::Properties;
    use super::Value;
    use oci_spec::runtime as oci;

    #[test]
    fn test_subsystem_workflow() {
        let mut pids_resources = oci::LinuxPids::default();
        pids_resources.set_limit(0 as i64);

        let mut properties: Properties = vec![];

        assert_eq!(true, Pids::apply(&pids_resources, &mut properties).is_ok());

        assert_eq!(Value::U64(u64::MAX), properties[0].1);
    }
}
