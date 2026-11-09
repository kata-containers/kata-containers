// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2022-2023 Nubificus LTD
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;
use std::sync::Arc;

use super::{default, register_hypervisor_plugin};

use crate::config::default::MAX_FIRECRACKER_VCPUS;
use crate::config::default::MIN_FIRECRACKER_MEMORY_SIZE_MB;

use crate::config::{ConfigPlugin, TomlConfig};
use crate::validate_path;

/// Hypervisor name for firecracker, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_FIRECRACKER: &str = "firecracker";

/// Configuration information for firecracker.
#[derive(Default, Debug)]
pub struct FirecrackerConfig {}

impl FirecrackerConfig {
    /// Create a new instance of `FirecrackerConfig`.
    pub fn new() -> Self {
        FirecrackerConfig {}
    }

    /// Register the firecracker plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_FIRECRACKER, plugin);
    }
}

impl ConfigPlugin for FirecrackerConfig {
    fn get_max_cpus(&self) -> u32 {
        MAX_FIRECRACKER_VCPUS
    }

    fn get_min_memory(&self) -> u32 {
        MIN_FIRECRACKER_MEMORY_SIZE_MB
    }

    fn name(&self) -> &str {
        HYPERVISOR_NAME_FIRECRACKER
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut TomlConfig) -> Result<()> {
        if let Some(firecracker) = conf.hypervisor.get_mut(HYPERVISOR_NAME_FIRECRACKER) {
            if firecracker.boot_info.kernel.is_empty() {
                firecracker.boot_info.kernel =
                    default::DEFAULT_FIRECRACKER_GUEST_KERNEL_IMAGE.to_string();
            }
            if firecracker.boot_info.kernel_params.is_empty() {
                firecracker.boot_info.kernel_params =
                    default::DEFAULT_FIRECRACKER_GUEST_KERNEL_PARAMS.to_string();
            }
            if firecracker.machine_info.entropy_source.is_empty() {
                firecracker.machine_info.entropy_source =
                    default::DEFAULT_FIRECRACKER_ENTROPY_SOURCE.to_string();
            }

            if firecracker.memory_info.default_memory == 0 {
                firecracker.memory_info.default_memory =
                    default::DEFAULT_FIRECRACKER_MEMORY_SIZE_MB;
            }
        }

        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &TomlConfig) -> Result<()> {
        if let Some(firecracker) = conf.hypervisor.get(HYPERVISOR_NAME_FIRECRACKER) {
            if firecracker.path.is_empty() {
                return Err(std::io::Error::other("Firecracker path is empty"));
            }
            validate_path!(
                firecracker.path,
                "FIRECRACKER binary path `{}` is invalid: {}"
            )?;
            if firecracker.boot_info.kernel.is_empty() {
                return Err(std::io::Error::other(
                    "Guest kernel image for firecracker is empty",
                ));
            }
            if firecracker.boot_info.image.is_empty() {
                return Err(std::io::Error::other(
                    "Both guest boot image and initrd for firecracker are empty",
                ));
            }

            if firecracker.cpu_info.default_vcpus > default::MAX_FIRECRACKER_VCPUS as f32
                || firecracker.cpu_info.default_maxvcpus > default::MAX_FIRECRACKER_VCPUS
            {
                return Err(std::io::Error::other(format!(
                    "Firecracker hypervisor can not support {} vCPUs",
                    firecracker.cpu_info.default_maxvcpus,
                )));
            }

            if firecracker.memory_info.default_memory < MIN_FIRECRACKER_MEMORY_SIZE_MB {
                return Err(std::io::Error::other(format!(
                    "Firecracker hypervisor has minimal memory limitation {MIN_FIRECRACKER_MEMORY_SIZE_MB}",
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
    use tempfile::NamedTempFile;

    #[test]
    fn test_validate_cpu_limits() {
        let plugin = FirecrackerConfig::new();
        let limit = default::MAX_FIRECRACKER_VCPUS;
        let binary = NamedTempFile::new().unwrap();
        let mut firecracker = Hypervisor {
            path: binary.path().to_string_lossy().into_owned(),
            ..Default::default()
        };
        firecracker.boot_info.kernel = "vmlinuz".to_string();
        firecracker.boot_info.image = "kata.img".to_string();
        firecracker.memory_info.default_memory = MIN_FIRECRACKER_MEMORY_SIZE_MB;

        for (boot_vcpus, max_vcpus, valid) in [
            (limit as f32 - 0.5, limit, true),
            (limit as f32, limit, true),
            (limit as f32 + 0.5, limit, false),
            (limit as f32 + 1.0, limit, false),
            (1.0, limit + 1, false),
        ] {
            firecracker.cpu_info.default_vcpus = boot_vcpus;
            firecracker.cpu_info.default_maxvcpus = max_vcpus;
            let mut config = TomlConfig::default();
            config.hypervisor.insert(
                HYPERVISOR_NAME_FIRECRACKER.to_string(),
                firecracker.clone(),
            );

            assert_eq!(
                plugin.validate(&config).is_ok(),
                valid,
                "boot_vcpus={boot_vcpus}, max_vcpus={max_vcpus}"
            );
        }
    }
}
