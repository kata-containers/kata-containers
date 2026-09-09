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

use crate::config::hypervisor::VIRTIO_BLK_MMIO;
use crate::config::{ConfigPlugin, TomlConfig};
use crate::{resolve_path, validate_path};

/// Hypervisor name for CH, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_CH: &str = "clh";

/// Configuration information for CH.
#[derive(Default, Debug)]
pub struct CloudHypervisorConfig {}

impl CloudHypervisorConfig {
    /// Maximum vCPUs for the host's Cloud Hypervisor backend.
    pub fn max_vcpus() -> u32 {
        if Path::new("/dev/mshv").exists() {
            // Preserve the existing MSHV limit; the larger guest is for KVM.
            default::MAX_CH_MSHV_VCPUS
        } else {
            MAX_CH_VCPUS
        }
    }

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
        Self::max_vcpus()
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
            if ch.factory.template_path.is_empty() {
                ch.factory.template_path = default::DEFAULT_TEMPLATE_PATH.to_string();
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
                return Err(std::io::Error::other("Path for CH jailer should be empty"));
            }
            if !ch.valid_jailer_paths.is_empty() {
                return Err(std::io::Error::other(
                    "Valid CH jailer path list should be empty",
                ));
            }

            // CoCo guest hardening: virtio-mmio is not hardened for confidential computing.
            if ch.security_info.confidential_guest
                && ch.boot_info.vm_rootfs_driver == VIRTIO_BLK_MMIO
            {
                return Err(std::io::Error::other(
                    "Confidential guests must not use virtio-blk-mmio (use virtio-blk-pci); \
                     virtio-mmio is not hardened for CoCo",
                ));
            }

            if ch.boot_info.kernel.is_empty() {
                return Err(std::io::Error::other("Guest kernel image for CH is empty"));
            }
            if ch.boot_info.image.is_empty() && ch.boot_info.initrd.is_empty() {
                return Err(std::io::Error::other(
                    "Both guest boot image and initrd for CH are empty",
                ));
            }

            let max_vcpus = Self::max_vcpus();
            if (ch.cpu_info.default_vcpus > 0.0 && ch.cpu_info.default_vcpus as u32 > max_vcpus)
                || ch.cpu_info.default_maxvcpus > max_vcpus
            {
                return Err(std::io::Error::other(format!(
                    "CH hypervisor cannot support {} vCPUs",
                    ch.cpu_info.default_maxvcpus,
                )));
            }

            if ch.device_info.default_bridges > default::MAX_CH_PCI_BRIDGES {
                return Err(std::io::Error::other(format!(
                    "CH hypervisor cannot support {} PCI bridges",
                    ch.device_info.default_bridges,
                )));
            }

            if ch.memory_info.default_memory < MIN_CH_MEMORY_SIZE_MB {
                return Err(std::io::Error::other(format!(
                    "CH hypervisor has minimal memory limitation {MIN_CH_MEMORY_SIZE_MB}",
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hypervisor::Hypervisor;

    #[test]
    fn test_validate_cpu_limits() {
        let plugin = CloudHypervisorConfig::new();
        let limit = if cfg!(target_arch = "x86_64") && !Path::new("/dev/mshv").exists() {
            512
        } else {
            256
        };
        let mut ch = Hypervisor::default();
        ch.boot_info.kernel = "vmlinuz".to_string();
        ch.boot_info.image = "kata.img".to_string();
        ch.memory_info.default_memory = MIN_CH_MEMORY_SIZE_MB;

        for (boot_vcpus, max_vcpus, valid) in [
            (limit, limit, true),
            (limit + 1, limit, false),
            (1, limit + 1, false),
        ] {
            ch.cpu_info.default_vcpus = boot_vcpus as f32;
            ch.cpu_info.default_maxvcpus = max_vcpus;
            let mut config = TomlConfig::default();
            config
                .hypervisor
                .insert(HYPERVISOR_NAME_CH.to_string(), ch.clone());

            assert_eq!(
                plugin.validate(&config).is_ok(),
                valid,
                "boot_vcpus={boot_vcpus}, max_vcpus={max_vcpus}"
            );
        }
    }
}
