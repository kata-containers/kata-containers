// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use common::error::Error as CommonError;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use ring::digest::{Context as Sha256Context, SHA256};
use serde::{Deserialize, Serialize};

use crate::factory::vm::VmConfig;
use crate::sandbox::VirtSandbox;

pub(super) const RUNTIME_ASSET_IDENTITY_OPTION: &str =
    "io.katacontainers.checkpoint.runtime-assets";

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAssetIdentity {
    // Resolve the caller's identity against this deployment, not the old path.
    #[serde(skip_serializing)]
    path: PathBuf,
    size: u64,
    sha256: String,
}

fn runtime_asset_identities(
    options: &HashMap<String, String>,
    config: &HypervisorConfig,
) -> Result<BTreeMap<&'static str, RuntimeAssetIdentity>> {
    let Some(value) = options.get(RUNTIME_ASSET_IDENTITY_OPTION) else {
        return Ok(BTreeMap::new());
    };

    let entries: Vec<RuntimeAssetIdentity> = serde_json::from_str(value).map_err(|err| {
        CommonError::InvalidSandboxOperation(format!("decode runtime asset identities: {err}"))
    })?;
    let mut by_path = HashMap::with_capacity(entries.len());
    for entry in entries {
        if !entry.path.is_absolute()
            || entry.size == 0
            || entry.sha256.len() != 64
            || !entry
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || by_path.insert(entry.path.clone(), entry).is_some()
        {
            return Err(CommonError::InvalidSandboxOperation(
                "invalid or duplicate runtime asset identity".to_string(),
            )
            .into());
        }
    }

    if by_path.is_empty() {
        return Ok(BTreeMap::new());
    }

    let paths = [
        ("hypervisor", config.path.as_str()),
        ("kernel", config.boot_info.kernel.as_str()),
        ("initrd", config.boot_info.initrd.as_str()),
        ("image", config.boot_info.image.as_str()),
    ];
    let expected: HashSet<_> = paths
        .iter()
        .filter(|(_, path)| !path.is_empty())
        .map(|(_, path)| Path::new(path))
        .collect();
    if by_path.len() != expected.len()
        || by_path
            .keys()
            .any(|path| !expected.contains(path.as_path()))
    {
        return Err(CommonError::InvalidSandboxOperation(
            "runtime asset identities do not match the active runtime configuration".to_string(),
        )
        .into());
    }

    // Compare roles and supplied identities across sandbox-specific directories.
    // The asset manager owns content verification and the binding to these files.
    Ok(paths
        .iter()
        .copied()
        .filter_map(|(role, path)| {
            by_path
                .get(Path::new(path))
                .cloned()
                .map(|entry| (role, entry))
        })
        .collect())
}

#[derive(Serialize)]
pub(super) struct RuntimeIdentity {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    assets: BTreeMap<&'static str, RuntimeAssetIdentity>,
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

fn runtime_config_fingerprint(
    config: HypervisorConfig,
    options: &HashMap<String, String>,
) -> Result<String> {
    let assets = runtime_asset_identities(options, &config)?;
    let has_assets = !assets.is_empty();
    let identity = RuntimeIdentity {
        assets,
        architecture: std::env::consts::ARCH.to_string(),
        hypervisor: if has_assets {
            String::new()
        } else {
            config.path
        },
        kernel: if has_assets {
            String::new()
        } else {
            config.boot_info.kernel
        },
        initrd: if has_assets {
            String::new()
        } else {
            config.boot_info.initrd
        },
        image: if has_assets {
            String::new()
        } else {
            config.boot_info.image
        },
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
        runtime_fingerprint: &str,
        options: &HashMap<String, String>,
    ) -> Result<()> {
        let actual = self.runtime_fingerprint(options).await?;
        if runtime_fingerprint != actual {
            return Err(CommonError::SandboxOperationPrecondition(
                "checkpoint runtime fingerprint does not match this runtime".to_string(),
            )
            .into());
        }

        Ok(())
    }

    pub(super) async fn runtime_fingerprint(
        &self,
        options: &HashMap<String, String>,
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

        runtime_config_fingerprint(config, options)
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
        let first = runtime_config_fingerprint(config.clone(), &HashMap::new()).unwrap();
        assert_eq!(
            first,
            runtime_config_fingerprint(config, &HashMap::new()).unwrap()
        );
    }

    #[test]
    fn runtime_fingerprint_detects_configuration_changes() {
        let config = config();
        let expected = runtime_config_fingerprint(config.clone(), &HashMap::new()).unwrap();
        for case in 0..4 {
            let mut changed = config.clone();
            match case {
                0 => changed.memory_info.default_memory += 1,
                1 => changed.cpu_info.default_maxvcpus += 1,
                2 => changed.boot_info.image.push_str(".other"),
                _ => changed.blockdev_info.block_device_driver.push_str(".other"),
            }
            assert_ne!(
                expected,
                runtime_config_fingerprint(changed, &HashMap::new()).unwrap()
            );
        }
    }

    fn options(config: &HypervisorConfig) -> HashMap<String, String> {
        let entries: Vec<_> = [
            &config.path,
            &config.boot_info.kernel,
            &config.boot_info.image,
        ]
        .iter()
        .enumerate()
        .map(|(role, path)| {
            serde_json::json!({
                "path": path, "size": role + 1, "sha256": format!("{:064x}", role + 1)
            })
        })
        .collect();
        HashMap::from([(
            RUNTIME_ASSET_IDENTITY_OPTION.to_string(),
            serde_json::to_string(&entries).unwrap(),
        )])
    }

    #[test]
    fn supplied_asset_identity_survives_path_and_order_changes() {
        let first = config();
        let expected = runtime_config_fingerprint(first.clone(), &options(&first)).unwrap();
        let mut relocated = first;
        relocated.path.push_str(".relocated");
        relocated.boot_info.kernel.push_str(".relocated");
        relocated.boot_info.image.push_str(".relocated");
        let mut identities = options(&relocated);
        let mut entries: Vec<serde_json::Value> =
            serde_json::from_str(&identities[RUNTIME_ASSET_IDENTITY_OPTION]).unwrap();
        entries.reverse();
        identities.insert(
            RUNTIME_ASSET_IDENTITY_OPTION.into(),
            serde_json::to_string(&entries).unwrap(),
        );
        assert_eq!(
            expected,
            runtime_config_fingerprint(relocated, &identities).unwrap()
        );
    }

    #[test]
    fn supplied_asset_identity_mismatch_is_not_ignored() {
        let config = config();
        let identities = options(&config);
        let expected = runtime_config_fingerprint(config.clone(), &identities).unwrap();
        assert_ne!(
            expected,
            runtime_config_fingerprint(config.clone(), &HashMap::new()).unwrap()
        );
        for field in ["size", "sha256"] {
            let mut entries: Vec<serde_json::Value> =
                serde_json::from_str(&identities[RUNTIME_ASSET_IDENTITY_OPTION]).unwrap();
            entries[0][field] = if field == "size" {
                serde_json::json!(999)
            } else {
                serde_json::json!("f".repeat(64))
            };
            let changed = HashMap::from([(
                RUNTIME_ASSET_IDENTITY_OPTION.into(),
                serde_json::to_string(&entries).unwrap(),
            )]);
            assert_ne!(
                expected,
                runtime_config_fingerprint(config.clone(), &changed).unwrap()
            );
        }
    }

    #[test]
    fn supplied_asset_inventory_must_match_configuration() {
        let config = config();
        let identities = options(&config);
        let valid: Vec<serde_json::Value> =
            serde_json::from_str(&identities[RUNTIME_ASSET_IDENTITY_OPTION]).unwrap();
        for case in 0..7 {
            let mut entries = valid.clone();
            match case {
                0 => {
                    entries.pop();
                }
                1 => entries.push(entries[0].clone()),
                2 => entries[0]["path"] = serde_json::json!("/wrong/vmm"),
                3 => entries[0]["path"] = serde_json::json!("relative/vmm"),
                4 => entries[0]["size"] = serde_json::json!(0),
                5 => entries[0]["sha256"] = serde_json::json!("not-a-digest"),
                _ => entries[0]["unknown"] = serde_json::json!(true),
            }
            let invalid = HashMap::from([(
                RUNTIME_ASSET_IDENTITY_OPTION.into(),
                serde_json::to_string(&entries).unwrap(),
            )]);
            assert!(
                runtime_config_fingerprint(config.clone(), &invalid).is_err(),
                "case {}",
                case
            );
        }
        let malformed = HashMap::from([(RUNTIME_ASSET_IDENTITY_OPTION.into(), "{".into())]);
        assert!(runtime_config_fingerprint(config, &malformed).is_err());
    }
}
