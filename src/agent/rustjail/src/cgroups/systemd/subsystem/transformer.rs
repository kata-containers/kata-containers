// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use super::super::common::{CgroupHierarchy, Properties};
use anyhow::Result;
use oci::LinuxResources;
use oci_spec::runtime as oci;

pub trait Transformer {
    fn apply(
        r: &LinuxResources,
        properties: &mut Properties,
        cgroup_hierarchy: &CgroupHierarchy,
        systemd_version: &str,
    ) -> Result<()>;
}
