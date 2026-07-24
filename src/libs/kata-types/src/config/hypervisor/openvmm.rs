// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;
use std::path::Path;
use std::sync::Arc;

use crate::config::{ConfigPlugin, TomlConfig};
use crate::validate_path;

use super::register_hypervisor_plugin;

/// Hypervisor name for openvmm, used to index `TomlConfig::hypervisor`.
pub const HYPERVISOR_NAME_OPENVMM: &str = "openvmm";

/// Maximum number of vCPUs for openvmm.
pub const MAX_OPENVMM_VCPUS: u32 = 256;

/// Minimum memory size in MiB for openvmm.
pub const MIN_OPENVMM_MEMORY_SIZE_MB: u32 = 64;

/// Default memory slots for openvmm.
pub const DEFAULT_OPENVMM_MEMORY_SLOTS: u32 = 128;

/// Configuration information for openvmm.
#[derive(Default, Debug)]
pub struct OpenVmmConfig {}

impl OpenVmmConfig {
    /// Create a new instance of `OpenVmmConfig`.
    pub fn new() -> Self {
        OpenVmmConfig {}
    }

    /// Register the openvmm plugin.
    pub fn register(self) {
        let plugin = Arc::new(self);
        register_hypervisor_plugin(HYPERVISOR_NAME_OPENVMM, plugin);
    }
}

impl ConfigPlugin for OpenVmmConfig {
    fn get_max_cpus(&self) -> u32 {
        MAX_OPENVMM_VCPUS
    }

    fn get_min_memory(&self) -> u32 {
        MIN_OPENVMM_MEMORY_SIZE_MB
    }

    fn name(&self) -> &str {
        HYPERVISOR_NAME_OPENVMM
    }

    /// Adjust the configuration information after loading from configuration file.
    fn adjust_config(&self, conf: &mut TomlConfig) -> Result<()> {
        if let Some(ovmm) = conf.hypervisor.get_mut(HYPERVISOR_NAME_OPENVMM) {
            if ovmm.memory_info.memory_slots == 0 {
                ovmm.memory_info.memory_slots = DEFAULT_OPENVMM_MEMORY_SLOTS;
            }
        }
        Ok(())
    }

    /// Validate the configuration information.
    fn validate(&self, conf: &TomlConfig) -> Result<()> {
        if let Some(ovmm) = conf.hypervisor.get(HYPERVISOR_NAME_OPENVMM) {
            if ovmm.path.is_empty() {
                return Err(std::io::Error::other("OpenVMM binary path is empty"));
            }
            validate_path!(ovmm.path, "OpenVMM binary path `{}` is invalid: {}")?;

            if (ovmm.cpu_info.default_vcpus > 0.0
                && ovmm.cpu_info.default_vcpus as u32 > MAX_OPENVMM_VCPUS)
                || ovmm.cpu_info.default_maxvcpus > MAX_OPENVMM_VCPUS
            {
                return Err(std::io::Error::other(format!(
                    "OpenVMM hypervisor cannot support more than {MAX_OPENVMM_VCPUS} vCPUs",
                )));
            }

            if ovmm.memory_info.default_memory < MIN_OPENVMM_MEMORY_SIZE_MB {
                return Err(std::io::Error::other(format!(
                    "OpenVMM hypervisor has minimal memory limitation {MIN_OPENVMM_MEMORY_SIZE_MB}",
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

    fn create_config(path: &Path) -> TomlConfig {
        let hypervisor = Hypervisor {
            path: path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let mut config = TomlConfig::default();
        config
            .hypervisor
            .insert(HYPERVISOR_NAME_OPENVMM.to_string(), hypervisor);
        config
    }

    #[test]
    fn adjust_config_supplies_memory_slots_only() {
        let binary = NamedTempFile::new().unwrap();
        let mut config = create_config(binary.path());
        let hypervisor = config.hypervisor.get_mut(HYPERVISOR_NAME_OPENVMM).unwrap();
        hypervisor.cpu_info.default_maxvcpus = MAX_OPENVMM_VCPUS + 1;

        OpenVmmConfig::new().adjust_config(&mut config).unwrap();

        let hypervisor = config.hypervisor.get(HYPERVISOR_NAME_OPENVMM).unwrap();
        assert_eq!(hypervisor.memory_info.default_memory, 0);
        assert_eq!(
            hypervisor.memory_info.memory_slots,
            DEFAULT_OPENVMM_MEMORY_SLOTS
        );
        assert_eq!(hypervisor.cpu_info.default_maxvcpus, MAX_OPENVMM_VCPUS + 1);
    }

    #[test]
    fn validate_accepts_supported_config() {
        let binary = NamedTempFile::new().unwrap();
        let mut config = create_config(binary.path());
        let hypervisor = config.hypervisor.get_mut(HYPERVISOR_NAME_OPENVMM).unwrap();
        hypervisor.cpu_info.default_vcpus = 1.0;
        hypervisor.cpu_info.default_maxvcpus = MAX_OPENVMM_VCPUS;
        hypervisor.memory_info.default_memory = MIN_OPENVMM_MEMORY_SIZE_MB;

        let plugin = OpenVmmConfig::new();
        plugin.adjust_config(&mut config).unwrap();
        plugin.validate(&config).unwrap();
        let hypervisor = config.hypervisor.get(HYPERVISOR_NAME_OPENVMM).unwrap();
        hypervisor.memory_info.validate().unwrap();
    }

    #[test]
    fn validate_rejects_empty_path() {
        let mut config = create_config(Path::new(""));
        config
            .hypervisor
            .get_mut(HYPERVISOR_NAME_OPENVMM)
            .unwrap()
            .memory_info
            .default_memory = MIN_OPENVMM_MEMORY_SIZE_MB;

        assert!(OpenVmmConfig::new().validate(&config).is_err());
    }

    #[test]
    fn validate_rejects_unsupported_vcpu_count() {
        let binary = NamedTempFile::new().unwrap();
        let mut config = create_config(binary.path());
        let hypervisor = config.hypervisor.get_mut(HYPERVISOR_NAME_OPENVMM).unwrap();
        hypervisor.cpu_info.default_maxvcpus = MAX_OPENVMM_VCPUS + 1;
        hypervisor.memory_info.default_memory = MIN_OPENVMM_MEMORY_SIZE_MB;

        assert!(OpenVmmConfig::new().validate(&config).is_err());
    }

    #[test]
    fn validate_rejects_insufficient_memory() {
        let binary = NamedTempFile::new().unwrap();
        let mut config = create_config(binary.path());
        config
            .hypervisor
            .get_mut(HYPERVISOR_NAME_OPENVMM)
            .unwrap()
            .memory_info
            .default_memory = MIN_OPENVMM_MEMORY_SIZE_MB - 1;

        assert!(OpenVmmConfig::new().validate(&config).is_err());
    }
}
