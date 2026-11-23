// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::Result;
use common::error::Error as CommonError;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use ring::digest::{Context as Sha256Context, SHA256};
use serde::Serialize;

use crate::factory::vm::VmConfig;
use crate::sandbox::VirtSandbox;

use super::manifest::CheckpointManifest;

#[derive(Serialize)]
pub(super) struct RuntimeIdentity {
    pub(super) architecture: String,
    pub(super) hypervisor: String,
    pub(super) kernel: String,
    pub(super) initrd: String,
    pub(super) image: String,
    pub(super) machine_type: String,
    pub(super) machine_accelerators: String,
    pub(super) cpu_features: String,
    pub(super) default_vcpus_bits: u32,
    pub(super) default_maxvcpus: u32,
    pub(super) default_memory: u32,
    pub(super) default_maxmemory: u32,
    pub(super) memory_slots: u32,
    pub(super) block_device_driver: String,
    pub(super) shared_fs: Option<String>,
}

fn runtime_config_fingerprint(config: HypervisorConfig) -> Result<String> {
    let identity = RuntimeIdentity {
        architecture: std::env::consts::ARCH.to_string(),
        hypervisor: config.path,
        kernel: config.boot_info.kernel,
        initrd: config.boot_info.initrd,
        image: config.boot_info.image,
        machine_type: config.machine_info.machine_type,
        machine_accelerators: config.machine_info.machine_accelerators,
        cpu_features: config.cpu_info.cpu_features,
        default_vcpus_bits: config.cpu_info.default_vcpus.to_bits(),
        default_maxvcpus: config.cpu_info.default_maxvcpus,
        default_memory: config.memory_info.default_memory,
        default_maxmemory: config.memory_info.default_maxmemory,
        memory_slots: config.memory_info.memory_slots,
        block_device_driver: config.blockdev_info.block_device_driver,
        shared_fs: config.shared_fs.shared_fs,
    };

    let mut hasher = Sha256Context::new(&SHA256);
    hasher.update(&serde_json::to_vec(&identity)?);
    let digest = hasher.finish();

    Ok(digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

impl VirtSandbox {
    pub(super) async fn verify_runtime_fingerprint(
        &self,
        manifest: &CheckpointManifest,
        options: &HashMap<String, String>,
    ) -> Result<()> {
        let actual = self.runtime_fingerprint(options).await?;
        if manifest.runtime_fingerprint != actual {
            return Err(CommonError::SandboxOperationPrecondition(
                "checkpoint runtime fingerprint does not match this runtime".to_string(),
            )
            .into());
        }

        Ok(())
    }

    pub(super) async fn runtime_fingerprint(
        &self,
        _options: &HashMap<String, String>,
    ) -> Result<String> {
        let config = self.hypervisor.hypervisor_config().await;
        if let Some(identity) = &self.template_identity {
            let template_config = VmConfig::new(self.resource_manager.config().await.as_ref());
            identity.verify_vm_config(&template_config).map_err(|err| {
                CommonError::SandboxOperationPrecondition(format!(
                    "VM template configuration is incompatible: {err:#}"
                ))
            })?;
        }

        runtime_config_fingerprint(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HypervisorConfig {
        // These files do not need to exist for configuration comparison.
        let mut config = HypervisorConfig {
            path: "/nonexistent/kata-runtime-assets/vmm".into(),
            ..Default::default()
        };
        config.boot_info.kernel = "/nonexistent/kata-runtime-assets/kernel".into();
        config.boot_info.image = "/nonexistent/kata-runtime-assets/image".into();
        config
    }

    #[test]
    fn runtime_fingerprint_requires_no_asset_files() {
        let config = config();
        let first = runtime_config_fingerprint(config.clone()).unwrap();
        assert_eq!(first, runtime_config_fingerprint(config).unwrap());
    }

    #[test]
    fn runtime_fingerprint_detects_configuration_changes() {
        let config = config();
        let expected = runtime_config_fingerprint(config.clone()).unwrap();
        for case in 0..4 {
            let mut changed = config.clone();
            match case {
                0 => changed.memory_info.default_memory += 1,
                1 => changed.cpu_info.default_maxvcpus += 1,
                2 => changed.boot_info.image.push_str(".other"),
                _ => changed.blockdev_info.block_device_driver.push_str(".other"),
            }
            assert_ne!(expected, runtime_config_fingerprint(changed).unwrap());
        }
    }
}
