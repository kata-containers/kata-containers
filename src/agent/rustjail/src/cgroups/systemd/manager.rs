// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::cgroups::Manager as CgroupManager;
use crate::cgroups_rs as cgroups;
use crate::protocols::agent::CgroupStats;
use anyhow::{anyhow, Result};
use cgroups::freezer::FreezerState;
use libc::{self, pid_t};
use oci::LinuxResources;
use oci_spec::runtime as oci;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::convert::TryInto;
use std::string::String;
use std::vec;

use super::super::fs::Manager as FsManager;

use super::cgroups_path::CgroupsPath;
use super::common::{CgroupHierarchy, Properties, DEFAULT_SLICE};
use super::dbus_client::{DBusClient, SystemdInterface};
use super::subsystem::transformer::Transformer;
use super::subsystem::{cpu::Cpu, cpuset::CpuSet, memory::Memory, pids::Pids};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manager {
    pub paths: HashMap<String, String>,
    pub mounts: HashMap<String, String>,
    pub cgroups_path: CgroupsPath,
    pub cpath: String,
    // dbus client for set properties
    dbus_client: DBusClient,
    // fs manager for get properties
    fs_manager: FsManager,
    // cgroup version for different dbus properties
    cg_hierarchy: CgroupHierarchy,
    // the sandbox memory bound to put on the pod's slice, 0 for none
    #[serde(default)]
    pod_memory_max_bytes: u64,
}

impl CgroupManager for Manager {
    fn apply(&self, pid: pid_t) -> Result<()> {
        if self.dbus_client.unit_exists()? {
            let subcgroup = self.fs_manager.subcgroup();
            self.dbus_client.add_process(pid, subcgroup)?;
        } else {
            self.dbus_client.start_unit(
                (pid as u32).try_into().unwrap(),
                self.cgroups_path.slice.as_str(),
                &self.cg_hierarchy,
            )?;
            self.set_pod_memory_max()?;
        }

        Ok(())
    }

    fn set(&self, r: &LinuxResources, _: bool) -> Result<()> {
        let mut properties: Properties = vec![];

        let systemd_version = self.dbus_client.get_version()?;
        let systemd_version_str = systemd_version.as_str();

        Cpu::apply(r, &mut properties, &self.cg_hierarchy, systemd_version_str)?;
        Memory::apply(r, &mut properties, &self.cg_hierarchy, systemd_version_str)?;
        Pids::apply(r, &mut properties, &self.cg_hierarchy, systemd_version_str)?;
        CpuSet::apply(r, &mut properties, &self.cg_hierarchy, systemd_version_str)?;

        self.dbus_client.set_properties(&properties)?;

        Ok(())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        self.fs_manager.get_stats()
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        match state {
            FreezerState::Thawed => self.dbus_client.thaw_unit(),
            FreezerState::Frozen => self.dbus_client.freeze_unit(),
            _ => Err(anyhow!("Invalid FreezerState")),
        }
    }

    fn destroy(&self) -> Result<()> {
        self.dbus_client.kill_unit()?;
        self.fs_manager.destroy()
    }

    fn get_pids(&self) -> Result<Vec<pid_t>> {
        self.fs_manager.get_pids()
    }

    fn update_cpuset_path(&self, guest_cpuset: &str, container_cpuset: &str) -> Result<()> {
        self.fs_manager
            .update_cpuset_path(guest_cpuset, container_cpuset)
    }

    fn get_cgroup_path(&self, cg: &str) -> Result<String> {
        self.fs_manager.get_cgroup_path(cg)
    }

    fn as_any(&self) -> Result<&dyn Any> {
        Ok(self)
    }

    fn name(&self) -> &str {
        "systemd"
    }
}

// pod_slice is the slice a bound on the pod's containers together goes on: the
// slice the host named, which is the pod's cgroup. A container in the default
// slice, or in the root, belongs to no pod.
fn pod_slice(slice: &str) -> Option<&str> {
    match slice {
        "" | "-.slice" | DEFAULT_SLICE => None,
        _ => Some(slice),
    }
}

impl Manager {
    pub fn new(cgroups_path_str: &str, pod_memory_max_bytes: u64) -> Result<Self> {
        let cgroups_path = CgroupsPath::new(cgroups_path_str)?;
        let (parent_slice, unit_name) = cgroups_path.parse()?;
        let cpath = parent_slice + "/" + &unit_name;

        let fs_manager = FsManager::new_systemd(cpath.as_str())?;

        Ok(Manager {
            paths: fs_manager.paths.clone(),
            mounts: fs_manager.mounts.clone(),
            cgroups_path,
            cpath,
            dbus_client: DBusClient::new(unit_name),
            fs_manager,
            cg_hierarchy: if cgroups::hierarchies::is_cgroup2_unified_mode() {
                CgroupHierarchy::Unified
            } else {
                CgroupHierarchy::Legacy
            },
            pod_memory_max_bytes,
        })
    }

    // set_pod_memory_max puts the sandbox memory bound on the pod's slice, the
    // parent of every container scope, once the first scope has brought the
    // slice up. Without a pod slice the containers keep their own limits.
    fn set_pod_memory_max(&self) -> Result<()> {
        if self.pod_memory_max_bytes == 0 {
            return Ok(());
        }
        let Some(slice) = pod_slice(self.cgroups_path.slice.as_str()) else {
            slog_scope::warn!(
                "Container in {} has no pod cgroup to bound its memory with the sandbox's other containers",
                self.cgroups_path.slice
            );
            return Ok(());
        };
        self.dbus_client.set_slice_memory_max(
            slice,
            self.pod_memory_max_bytes,
            &self.cg_hierarchy,
        )?;
        slog_scope::info!(
            "the sandbox memory bound is set on the pod's slice";
            "slice" => slice,
            "bytes" => self.pod_memory_max_bytes
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::pod_slice;

    #[test]
    fn pod_slice_is_the_slice_the_host_named() {
        assert_eq!(
            pod_slice("kubepods-pod0123.slice"),
            Some("kubepods-pod0123.slice")
        );
        assert_eq!(
            pod_slice("kubepods-burstable-pod0123.slice"),
            Some("kubepods-burstable-pod0123.slice")
        );
    }

    #[test]
    fn default_and_root_slices_are_no_pod() {
        assert_eq!(pod_slice("system.slice"), None);
        assert_eq!(pod_slice("-.slice"), None);
        assert_eq!(pod_slice(""), None);
    }
}
