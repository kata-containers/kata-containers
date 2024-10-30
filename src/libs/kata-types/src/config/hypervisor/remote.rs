// Copyright 2024 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use byte_unit::{Byte, Unit};
use std::io::Result;
use std::path::Path;
use std::sync::Arc;
use sysinfo::System;

use crate::{
    config::{
        default::{self, MAX_REMOTE_VCPUS, MIN_REMOTE_MEMORY_SIZE_MB},
        ConfigPlugin,
    }, device::DRIVER_NVDIMM_TYPE, eother, resolve_path
};

use super::register_hypervisor_plugin;

/// Hypervisor name for remote, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_REMOTE: &str = "remote";

/// Configuration information for remote.
#[derive(Default, Debug)]
pub struct RemoteConfig {}

impl RemoteConfig {
    /// Create a new instance of `RemoteConfig`
    pub fn new() -> Self {
        RemoteConfig {}
    }

    /// Register the remote plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_REMOTE, plugin);
    }
}

impl ConfigPlugin for RemoteConfig {
    fn name(&self) -> &str {
        HYPERVISOR_NAME_REMOTE
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut crate::config::TomlConfig) -> Result<()> {
        if let Some(remote) = conf.hypervisor.get_mut(HYPERVISOR_NAME_REMOTE) {
            if remote.remote_info.hypervisor_socket.is_empty() {
                remote.remote_info.hypervisor_socket =
                    default::DEFAULT_REMOTE_HYPERVISOR_SOCKET.to_string();
            }
            resolve_path!(
                remote.remote_info.hypervisor_socket,
                "Remote hypervisor socket `{}` is invalid: {}"
            )?;
            if remote.remote_info.hypervisor_timeout == 0 {
                remote.remote_info.hypervisor_timeout = default::DEFAULT_REMOTE_HYPERVISOR_TIMEOUT;
            }
            if remote.memory_info.default_memory == 0 {
                remote.memory_info.default_memory = default::MIN_REMOTE_MEMORY_SIZE_MB;
            }
            if remote.memory_info.memory_slots == 0 {
                remote.memory_info.memory_slots = default::DEFAULT_REMOTE_MEMORY_SLOTS
            }
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &crate::config::TomlConfig) -> Result<()> {
        if let Some(remote) = conf.hypervisor.get(HYPERVISOR_NAME_REMOTE) {
            let s = System::new_all();
            let total_memory = Byte::from_u64(s.total_memory())
                .get_adjusted_unit(Unit::MiB)
                .get_value() as u32;
            if remote.memory_info.default_maxmemory != total_memory {
                return Err(eother!(
                    "Remote hypervisor does not support memory hotplug, default_maxmemory must be equal to the total system memory",
                ));
            }
            let cpus = num_cpus::get() as u32;
            if remote.cpu_info.default_maxvcpus != cpus {
                return Err(eother!(
                    "Remote hypervisor does not support CPU hotplug, default_maxvcpus must be equal to the total system CPUs",
                ));
            }
            if !remote.boot_info.initrd.is_empty() {
                return Err(eother!("Remote hypervisor does not support initrd"));
            }
            if !remote.boot_info.rootfs_type.is_empty() {
                return Err(eother!("Remote hypervisor does not support rootfs_type"));
            }
            if remote.blockdev_info.block_device_driver.as_str() == DRIVER_NVDIMM_TYPE {
                return Err(eother!("Remote hypervisor does not support nvdimm"));
            }
            if remote.memory_info.default_memory < MIN_REMOTE_MEMORY_SIZE_MB {
                return Err(eother!(
                    "Remote hypervisor has minimal memory limitation {}",
                    MIN_REMOTE_MEMORY_SIZE_MB
                ));
            }
        }

        Ok(())
    }

    fn get_min_memory(&self) -> u32 {
        MIN_REMOTE_MEMORY_SIZE_MB
    }

    fn get_max_cpus(&self) -> u32 {
        MAX_REMOTE_VCPUS
    }
}
