// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::cgroups::Manager as CgroupManager;
use crate::protocols::agent::CgroupStats;
use anyhow::Result;
use cgroups::freezer::FreezerState;
use libc::{self, pid_t};
use oci::LinuxResources;
use std::any::Any;
use std::collections::HashMap;
use std::convert::TryInto;
use std::string::String;
use std::vec;

use super::super::fs::Manager as FsManager;

use super::cgroups_path::CgroupsPath;
use super::common::{CgroupHierarchy, Properties};
use super::dbus_client::{DBusClient, SystemdInterface};
use super::subsystem::transformer::Transformer;
use super::subsystem::{cpu::Cpu, cpuset::CpuSet, memory::Memory, pids::Pids};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manager {
    pub paths: HashMap<String, String>,
    pub mounts: HashMap<String, String>,
    pub cgroups_path: CgroupsPath,
    pub cpath: String,
    pub unit_name: String,
    // dbus client for set properties
    dbus_client: DBusClient,
    // fs manager for get properties
    fs_manager: FsManager,
    // cgroup version for different dbus properties
    cg_hierarchy: CgroupHierarchy,
}

impl CgroupManager for Manager {
    fn apply(&self, pid: pid_t) -> Result<()> {
        let unit_name = self.unit_name.as_str();
        if self.dbus_client.unit_exists(unit_name)? {
            self.dbus_client.add_process(pid, self.unit_name.as_str())?;
        } else {
            self.dbus_client.start_unit(
                (pid as u32).try_into().unwrap(),
                self.cgroups_path.slice.as_str(),
                self.unit_name.as_str(),
                &self.cg_hierarchy,
            )?;
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

        self.dbus_client
            .set_properties(self.unit_name.as_str(), &properties)?;

        Ok(())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        self.fs_manager.get_stats()
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        self.fs_manager.freeze(state)
    }

    fn destroy(&mut self) -> Result<()> {
        self.dbus_client.stop_unit(self.unit_name.as_str())?;
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

impl Manager {
    pub fn new(cgroups_path_str: &str) -> Result<Self> {
        let cgroups_path = CgroupsPath::new(cgroups_path_str)?;
        let (parent_slice, unit_name) = cgroups_path.parse()?;
        let cpath = parent_slice + "/" + &unit_name;

        let fs_manager = FsManager::new(cpath.as_str())?;

        Ok(Manager {
            paths: fs_manager.paths.clone(),
            mounts: fs_manager.mounts.clone(),
            cgroups_path,
            cpath,
            unit_name,
            dbus_client: DBusClient {},
            fs_manager,
            cg_hierarchy: if cgroups::hierarchies::is_cgroup2_unified_mode() {
                CgroupHierarchy::Unified
            } else {
                CgroupHierarchy::Legacy
            },
        })
    }
}
