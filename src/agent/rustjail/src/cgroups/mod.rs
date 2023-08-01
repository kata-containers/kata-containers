// Copyright (c) 2019,2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use core::fmt::Debug;
use oci::{LinuxDeviceCgroup, LinuxResources};
use protocols::agent::CgroupStats;
use std::any::Any;

use cgroups::freezer::FreezerState;

pub mod fs;
pub mod mock;
pub mod notifier;
pub mod systemd;

#[derive(Default, Debug)]
pub struct DevicesCgroupInfo {
    /// Indicate if the pod cgroup is initialized.
    inited: bool,
    /// Indicate if pod's devices cgroup is in whitelist mode. Returns true
    /// once one container requires `a *:* rwm` permission.
    whitelist: bool,
}

pub trait Manager {
    fn apply(&self, _pid: i32) -> Result<()> {
        Err(anyhow!("not supported!".to_string()))
    }

    fn get_pids(&self) -> Result<Vec<i32>> {
        Err(anyhow!("not supported!"))
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        Err(anyhow!("not supported!"))
    }

    fn freeze(&self, _state: FreezerState) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn destroy(&mut self) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn set(&self, _container: &LinuxResources, _update: bool) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn update_cpuset_path(&self, _: &str, _: &str) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn get_cgroup_path(&self, _: &str) -> Result<String> {
        Err(anyhow!("not supported!"))
    }

    fn as_any(&self) -> Result<&dyn Any> {
        Err(anyhow!("not supported!"))
    }

    fn name(&self) -> &str;
}

impl Debug for dyn Manager + Send + Sync {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Check if device cgroup is an all rule from OCI spec.
///
/// The formats representing all devices between OCI spec and cgroups-rs
/// are different.
/// - OCI spec: major: 0, minor: 0, type: "" (All devices), access: "rwm";
/// - Cgroups-rs: major: -1, minor: -1, type: "a", access: "rwm";
/// - Linux: a *:* rwm
#[inline]
fn is_all_devices_rule(dev_cgroup: &LinuxDeviceCgroup) -> bool {
    dev_cgroup.major.unwrap_or(0) == 0
        && dev_cgroup.minor.unwrap_or(0) == 0
        && (dev_cgroup.r#type.as_str() == "" || dev_cgroup.r#type.as_str() == "a")
        && dev_cgroup.access.contains('r')
        && dev_cgroup.access.contains('w')
        && dev_cgroup.access.contains('m')
}
