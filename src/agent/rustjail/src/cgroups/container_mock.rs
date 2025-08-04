// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::Result;
use cgroups::{CgroupPid, CgroupStats, FreezerState};
use oci_spec::runtime::{LinuxResources, Spec};

use crate::cgroups::SandboxCgroupManager;

#[derive(Debug, Default)]
pub struct ContainerCgroupManager {}

impl ContainerCgroupManager {
    pub fn new(_sandbox: Arc<SandboxCgroupManager>, _path: &str, _spec: &Spec) -> Result<Self> {
        Ok(Default::default())
    }
}

impl ContainerCgroupManager {
    pub fn set(&self, _resources: &LinuxResources) -> Result<()> {
        Ok(())
    }

    pub fn freeze(&self, _state: FreezerState) -> Result<()> {
        Ok(())
    }

    pub fn pids(&self) -> Result<Vec<CgroupPid>> {
        Ok(Vec::new())
    }

    pub fn serialize(&self) -> Result<String> {
        Ok(String::new())
    }

    pub fn add_thread(&self, _pid: CgroupPid) -> Result<()> {
        Ok(())
    }

    pub fn enable_cpus_topdown(&self, _cpus: &str) -> Result<()> {
        Ok(())
    }

    pub fn cgroup_path(&self, _subsystem: Option<&str>) -> Result<String> {
        Ok(String::new())
    }

    pub fn stats(&self) -> CgroupStats {
        CgroupStats::default()
    }

    pub fn destroy(&self) -> Result<()> {
        Ok(())
    }
}
