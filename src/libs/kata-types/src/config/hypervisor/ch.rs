// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;
use std::sync::Arc;

use super::{default, register_hypervisor_plugin};

use crate::config::default::MAX_CH_VCPUS;
use crate::config::default::MIN_CH_MEMORY_SIZE_MB;

use crate::config::{ConfigPlugin, TomlConfig};
use crate::{eother, resolve_path, validate_path};

/// Hypervisor name for CH, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_CH: &str = "cloud-hypervisor";

/// Configuration information for CH.
#[derive(Default, Debug)]
pub struct CloudHypervisorConfig {}

impl CloudHypervisorConfig {
    /// Create a new instance of `CloudHypervisorConfig`.
    pub fn new() -> Self {
        CloudHypervisorConfig {}
    }

    /// Register the CH plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_CH, plugin);
    }
}

impl ConfigPlugin for CloudHypervisorConfig {
    fn get_max_cpus(&self) -> u32 {
        MAX_CH_VCPUS
    }

    fn get_min_memory(&self) -> u32 {
        MIN_CH_MEMORY_SIZE_MB
    }

    fn name(&self) -> &str {
        HYPERVISOR_NAME_CH
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut TomlConfig) -> Result<()> {
        if let Some(ch) = conf.hypervisor.get_mut(HYPERVISOR_NAME_CH) {
            if ch.path.is_empty() {
                ch.path = default::DEFAULT_CH_BINARY_PATH.to_string();
            }
            resolve_path!(ch.path, "CH binary path `{}` is invalid: {}")?;
            if ch.ctlpath.is_empty() {
                ch.ctlpath = default::DEFAULT_CH_CONTROL_PATH.to_string();
            }
            resolve_path!(ch.ctlpath, "CH ctlpath `{}` is invalid: {}")?;

            if ch.boot_info.kernel.is_empty() {
                ch.boot_info.kernel = default::DEFAULT_CH_GUEST_KERNEL_IMAGE.to_string();
            }
            if ch.boot_info.kernel_params.is_empty() {
                ch.boot_info.kernel_params = default::DEFAULT_CH_GUEST_KERNEL_PARAMS.to_string();
            }
            if ch.boot_info.firmware.is_empty() {
                ch.boot_info.firmware = default::DEFAULT_CH_FIRMWARE_PATH.to_string();
            }

            if ch.device_info.default_bridges == 0 {
                ch.device_info.default_bridges = default::DEFAULT_CH_PCI_BRIDGES;
            }

            if ch.machine_info.entropy_source.is_empty() {
                ch.machine_info.entropy_source = default::DEFAULT_CH_ENTROPY_SOURCE.to_string();
            }

            if ch.memory_info.default_memory == 0 {
                ch.memory_info.default_memory = default::DEFAULT_CH_MEMORY_SIZE_MB;
            }
            if ch.memory_info.memory_slots == 0 {
                ch.memory_info.memory_slots = default::DEFAULT_CH_MEMORY_SLOTS;
            }
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &TomlConfig) -> Result<()> {
        if let Some(ch) = conf.hypervisor.get(HYPERVISOR_NAME_CH) {
            validate_path!(ch.path, "CH binary path `{}` is invalid: {}")?;
            validate_path!(ch.ctlpath, "CH control path `{}` is invalid: {}")?;
            if !ch.jailer_path.is_empty() {
                return Err(eother!("Path for CH jailer should be empty"));
            }
            if !ch.valid_jailer_paths.is_empty() {
                return Err(eother!("Valid CH jailer path list should be empty"));
            }

            if ch.boot_info.kernel.is_empty() {
                return Err(eother!("Guest kernel image for CH is empty"));
            }
            if ch.boot_info.image.is_empty() && ch.boot_info.initrd.is_empty() {
                return Err(eother!("Both guest boot image and initrd for CH are empty"));
            }

            if (ch.cpu_info.default_vcpus > 0
                && ch.cpu_info.default_vcpus as u32 > default::MAX_CH_VCPUS)
                || ch.cpu_info.default_maxvcpus > default::MAX_CH_VCPUS
            {
                return Err(eother!(
                    "CH hypervisor cannot support {} vCPUs",
                    ch.cpu_info.default_maxvcpus
                ));
            }

            if ch.device_info.default_bridges > default::MAX_CH_PCI_BRIDGES {
                return Err(eother!(
                    "CH hypervisor cannot support {} PCI bridges",
                    ch.device_info.default_bridges
                ));
            }

            if ch.memory_info.default_memory < MIN_CH_MEMORY_SIZE_MB {
                return Err(eother!(
                    "CH hypervisor has minimal memory limitation {}",
                    MIN_CH_MEMORY_SIZE_MB
                ));
            }
        }

        Ok(())
    }
}
