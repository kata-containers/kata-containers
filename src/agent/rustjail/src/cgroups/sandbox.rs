// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context, Result};
use cgroups::fs::hierarchies::is_cgroup2_unified_mode;
use cgroups::manager::is_systemd_cgroup;
use cgroups::stats::DeviceCgroupStat;
use cgroups::{FsManager, Manager};
use oci_spec::runtime::{LinuxResources, Spec};

use crate::cgroups::device::{
    allow_all_devices_in_cgroup, allow_default_devices_in_cgroup, has_oci_spec_allowed_all,
};

#[derive(Debug, Default)]
pub struct SandboxCgroupManager {
    inner: Arc<RwLock<SandboxCgroupManagerInner>>,
}

impl SandboxCgroupManager {
    /// Try to initialize the sandbox cgroup.
    ///
    /// # Arguments
    ///
    /// - `path`: The cgroup path for the pause container.
    /// - `spec`: The OCI runtime spec for the sandbox.
    pub fn try_init(&self, path: &str, spec: &Spec) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.try_init(path, spec)
    }

    /// If the sandbox cgroup exists.
    pub fn enable(&self) -> bool {
        let inner = self.inner.read();
        inner.map(|inner| inner.enable()).unwrap_or_default()
    }

    /// If the sandbox devices cgroup is enabled.
    pub fn enable_devcg(&self) -> bool {
        let inner = self.inner.read();
        inner.map(|inner| inner.enable_devcg()).unwrap_or_default()
    }

    /// If the sandbox devices cgroup has universal access.
    pub fn is_allowed_all_devices(&self) -> bool {
        let inner = self.inner.read();
        inner
            .map(|inner| inner.is_allowed_all_devices())
            .unwrap_or_default()
    }

    pub fn allow_all_devices(&self) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.allow_all_devices()
    }

    pub fn set(&self, resources: &LinuxResources) -> Result<()> {
        let mut inner = self
            .inner
            .write()
            .map_err(|err| anyhow!("write lock: {}", err))?;
        inner.set(resources)
    }
}

#[derive(Debug, Default)]
struct SandboxCgroupManagerInner {
    pause_container_inited: bool,
    systemd_cgroup: bool,
    cgroup_manager: Option<Box<dyn Manager>>,
}

impl SandboxCgroupManagerInner {
    fn enable_devcg(&self) -> bool {
        self.enable() && (!self.systemd_cgroup && !is_cgroup2_unified_mode())
    }

    fn try_init(&mut self, path: &str, spec: &Spec) -> Result<()> {
        if self.pause_container_inited {
            return Ok(());
        }

        // When we are reaching here, the sandbox cgroup MUST not have
        // existed.
        if self.enable() {
            return Err(anyhow!("sandbox cgroup is expected not to exist"));
        }

        self.pause_container_inited = true;
        self.systemd_cgroup = is_systemd_cgroup(path);

        // TODO: Sandbox cgroup limits are not available for systemd
        // cgroup.
        if self.systemd_cgroup {
            return Ok(());
        }

        let path = Path::new(path);
        let path = match path.parent() {
            Some(p) => p,
            // Skip if the parent of cgroup path is empty
            None => return Ok(()),
        };

        // Skip if the sandbox cgroup path is the root.
        if path == Path::new("/") || path == Path::new("") {
            return Ok(());
        }

        let base = path.to_string_lossy().to_string();
        let base = base.trim_start_matches("/");
        let mut manager = FsManager::new(base).context("create cgroup manager")?;
        manager.create_cgroups().context("create cgroup")?;

        if self.enable_devcg() {
            if has_oci_spec_allowed_all(spec) {
                allow_all_devices_in_cgroup(&mut manager).context("allow all devices")?;
            } else {
                allow_default_devices_in_cgroup(&mut manager).context("grant default access")?;
            }
        }

        self.cgroup_manager = Some(Box::new(manager));

        Ok(())
    }

    fn enable(&self) -> bool {
        self.cgroup_manager.is_some()
    }

    fn is_allowed_all_devices(&self) -> bool {
        fn has_allowed_all(list: Vec<DeviceCgroupStat>) -> bool {
            let mut dev_block = false;
            let mut dev_char = false;

            for item in list {
                let major = item.major;
                let minor = item.minor;
                let access = item.access.as_str();
                let dev_type = item.dev_type.as_str();
                if major != -1 || minor != -1 || access != "rwm" {
                    continue;
                }

                if dev_type == "a" {
                    return true; // Universal access
                } else if dev_type == "b" {
                    dev_block = true;
                } else if dev_type == "c" {
                    dev_char = true;
                }
            }

            dev_block && dev_char
        }

        self.cgroup_manager
            .as_ref()
            .map(|manager| manager.stats().devices.list)
            .map(has_allowed_all)
            .unwrap_or_default()
    }

    fn allow_all_devices(&mut self) -> Result<()> {
        if !self.enable_devcg() {
            return Ok(());
        }

        let manager = self
            .cgroup_manager
            .as_mut()
            .ok_or(anyhow!("no cgroup manager"))?;

        allow_all_devices_in_cgroup(manager.as_mut()).context("grant allowed all")?;

        Ok(())
    }

    fn set(&mut self, resources: &LinuxResources) -> Result<()> {
        let cgroup_manager = self
            .cgroup_manager
            .as_mut()
            .ok_or(anyhow!("no cgroup manager"))?;
        cgroup_manager.set(resources).context("set")?;

        Ok(())
    }
}
