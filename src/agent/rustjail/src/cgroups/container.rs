// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use cgroups::manager::is_systemd_cgroup;
use cgroups::{CgroupPid, CgroupStats, FreezerState, FsManager, Manager, SystemdManager};
use oci_spec::runtime::{LinuxResources, LinuxResourcesBuilder, Spec};

use crate::cgroups::device::{allow_default_devices_in_cgroup, has_oci_spec_allowed_all};
use crate::cgroups::sandbox::SandboxCgroupManager;

#[derive(Debug)]
pub struct ContainerCgroupManager {
    inner: Arc<RwLock<ContainerCgroupManagerInner>>,
}

impl ContainerCgroupManager {
    pub fn new(sandbox: Arc<SandboxCgroupManager>, path: &str, spec: &Spec) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ContainerCgroupManagerInner::new(
                sandbox, path, spec,
            )?)),
        })
    }
}

impl ContainerCgroupManager {
    pub fn set(&self, resources: &LinuxResources) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.set(resources)
    }

    pub fn freeze(&self, state: FreezerState) -> Result<()> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("read lock: {}", err))?;
        inner.freeze(state)
    }

    pub fn pids(&self) -> Result<Vec<CgroupPid>> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("read lock: {}", err))?;
        inner.pids()
    }

    pub fn serialize(&self) -> Result<String> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("read lock: {}", err))?;
        inner.serialize()
    }

    pub fn add_thread(&self, pid: CgroupPid) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.add_thread(pid)
    }

    pub fn enable_cpus_topdown(&self, cpus: &str) -> Result<()> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("read lock: {}", err))?;
        inner.enable_cpus_topdown(cpus)
    }

    pub fn cgroup_path(&self, subsystem: Option<&str>) -> Result<String> {
        let inner = self
            .inner
            .read()
            .map_err(|err| anyhow!("read lock: {}", err))?;
        inner.cgroup_path(subsystem)
    }

    pub fn stats(&self) -> CgroupStats {
        let inner = self.inner.read();
        inner.map(|inner| inner.stats()).unwrap_or_default()
    }

    pub fn destroy(&self) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.destroy()
    }
}

#[derive(Debug)]
struct ContainerCgroupManagerInner {
    sandbox_cgroup_manager: Arc<SandboxCgroupManager>,
    cgroup_manager: Box<dyn Manager>,
}

impl ContainerCgroupManagerInner {
    pub fn new(sandbox: Arc<SandboxCgroupManager>, path: &str, spec: &Spec) -> Result<Self> {
        sandbox
            .try_init(path, spec)
            .context("try init sandbox cgroup manager")?;

        let sandbox_devcg = sandbox.enable_devcg();
        let container_allowed_all = has_oci_spec_allowed_all(spec);
        if sandbox_devcg {
            let sandbox_allowed_all = sandbox.is_allowed_all_devices();

            // The upper node of container cgroup tree (a.k.a sandbox
            // cgroup) should be superset of the container devices cgroup.
            if !sandbox_allowed_all && container_allowed_all {
                sandbox.allow_all_devices()?;
            }
        }

        let cgroup_manager: Box<dyn Manager> = if is_systemd_cgroup(path) {
            Box::new(SystemdManager::new(path).context("new systemd manager")?)
        } else {
            let path = path.trim_start_matches("/");
            let mut manager = FsManager::new(path).context("new fs manager")?;
            if sandbox_devcg {
                // We need to create the cgroups immediately.
                manager.create_cgroups().context("create cgroups")?;
                // The permissions are inherited from the sandbox, which
                // might contain some ones that are not allowed for this
                // container. Therefore, we need to reset the permissions
                // to the default ones for the container.
                if !container_allowed_all {
                    allow_default_devices_in_cgroup(&mut manager)
                        .context("grant default permissions")?;
                }
            }

            Box::new(manager)
        };

        Ok(Self {
            sandbox_cgroup_manager: sandbox,
            cgroup_manager,
        })
    }
}

impl ContainerCgroupManagerInner {
    fn set(&mut self, resources: &LinuxResources) -> Result<()> {
        // Set resources for the sandbox first
        let sandbox_devcg = self.sandbox_cgroup_manager.enable_devcg();
        let sandbox_allowed_all = self.sandbox_cgroup_manager.is_allowed_all_devices();
        if sandbox_devcg {
            let mut builder = LinuxResourcesBuilder::default();

            if !sandbox_allowed_all {
                if let Some(devices) = resources.devices() {
                    builder = builder.devices(devices.clone());
                }
            }

            let resources = builder.build().context("build LinuxResources")?;
            if resources != LinuxResources::default() {
                self.sandbox_cgroup_manager
                    .set(&resources)
                    .context("set sandbox resources")?;
            }
        }

        // The sandbox is all set, now we can set the container cgroup
        // resources.
        self.cgroup_manager
            .as_mut()
            .set(resources)
            .context("set container cgroup resources")?;

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        self.cgroup_manager
            .as_ref()
            .freeze(state)
            .context("freeze container cgroup")
    }

    fn pids(&self) -> Result<Vec<CgroupPid>> {
        self.cgroup_manager
            .as_ref()
            .pids()
            .context("get pids from cgroup manager")
    }

    fn serialize(&self) -> Result<String> {
        self.cgroup_manager
            .serialize()
            .context("serialize cgroup manager")
    }

    fn add_thread(&mut self, pid: CgroupPid) -> Result<()> {
        self.cgroup_manager
            .as_mut()
            .add_thread(pid)
            .context("add thread to cgroup manager")
    }

    fn enable_cpus_topdown(&self, cpus: &str) -> Result<()> {
        self.cgroup_manager
            .as_ref()
            .enable_cpus_topdown(cpus)
            .context("enable cpus topdown")
    }

    fn cgroup_path(&self, subsystem: Option<&str>) -> Result<String> {
        self.cgroup_manager
            .as_ref()
            .cgroup_path(subsystem)
            .context("get cgroup path")
    }

    fn stats(&self) -> CgroupStats {
        self.cgroup_manager.as_ref().stats()
    }

    fn destroy(&mut self) -> Result<()> {
        self.cgroup_manager
            .as_mut()
            .destroy()
            .context("destroy cgroup manager")
    }
}
