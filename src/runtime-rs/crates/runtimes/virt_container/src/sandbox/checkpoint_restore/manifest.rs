// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::{fs, path::Path};

use anyhow::Result;
use common::error::Error as CommonError;
use serde::{Deserialize, Serialize};

use crate::factory::template::TemplateIdentity;
use crate::sandbox::checkpoint_restore::artifacts::vm::VmCheckpointArtifact;

use super::checkpoint_files;

pub(super) const WORKLOAD_CHECKPOINT_FORMAT_VERSION: u32 = 2;
pub(super) const WORKLOAD_CHECKPOINT_MANIFEST: &str = "manifest.json";
pub(super) const WORKLOAD_CHECKPOINT_STATE: &str = "vm";
pub(super) const WORKLOAD_CHECKPOINT_STORAGE: &str = "storage";

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointTask {
    pub(super) checkpoint_key: String,
    pub(super) task_id: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointRootfs {
    pub(super) checkpoint_key: String,
    pub(super) file: String,
    pub(super) format: String,
    pub(super) device_id: String,
    pub(super) index: u64,
    pub(super) driver_option: String,
    pub(super) virt_path: String,
    pub(super) is_direct: Option<bool>,
    pub(super) discard_unmap: bool,
    pub(super) aio: String,
    pub(super) queue_size: u32,
    pub(super) num_queues: usize,
    pub(super) logical_sector_size: u32,
    pub(super) physical_sector_size: u32,
    pub(super) serial_override: String,
    pub(super) logical_size: u64,
    pub(super) encoded_size: u64,
    pub(super) sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) immutable_lower: Option<ImmutableRootfs>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct ImmutableRootfs {
    pub(super) device_id: String,
    pub(super) index: u64,
    pub(super) driver_option: String,
    pub(super) virt_path: String,
    pub(super) is_direct: Option<bool>,
    pub(super) discard_unmap: bool,
    pub(super) aio: String,
    pub(super) queue_size: u32,
    pub(super) num_queues: usize,
    pub(super) logical_sector_size: u32,
    pub(super) physical_sector_size: u32,
    pub(super) serial_override: String,
    pub(super) size: u64,
    pub(super) sha256: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointManifest {
    pub(super) format_version: u32,
    pub(super) hypervisor: String,
    pub(super) tasks: Vec<CheckpointTask>,
    pub(super) rootfs: Vec<CheckpointRootfs>,
    pub(super) network_macs: Vec<[u8; 6]>,
    pub(super) vm_vcpus: u32,
    pub(super) vm_memory_mb: u32,
    pub(super) vm_state: Vec<VmCheckpointArtifact>,
    pub(super) runtime_fingerprint: String,
    #[serde(default)]
    pub(super) template: Option<TemplateIdentity>,
}

impl CheckpointManifest {
    pub(super) fn load(checkpoint_path: &str) -> Result<Self> {
        let (state_path, manifest_path) = checkpoint_files(checkpoint_path)?;
        let state_metadata = fs::symlink_metadata(&state_path)?;

        if state_metadata.file_type().is_symlink() || !state_metadata.is_dir() {
            return Err(CommonError::InvalidSandboxOperation(
                "VM checkpoint artifact path is not a directory".to_string(),
            )
            .into());
        }

        let manifest_metadata = fs::symlink_metadata(&manifest_path)?;
        if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint manifest is not a regular file".to_string(),
            )
            .into());
        }

        let mut manifest_file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&manifest_path)
            .map_err(|err| {
                CommonError::InvalidSandboxOperation(format!("open checkpoint manifest: {err}"))
            })?;

        let mut manifest_bytes = Vec::new();
        manifest_file
            .read_to_end(&mut manifest_bytes)
            .map_err(|err| {
                CommonError::InvalidSandboxOperation(format!("read checkpoint manifest: {err}"))
            })?;
        let manifest: CheckpointManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|err| {
                CommonError::InvalidSandboxOperation(format!("decode checkpoint manifest: {err}"))
            })?;

        Ok(manifest)
    }

    pub(super) fn publish(&self, path: &Path) -> Result<()> {
        let partial = path.with_extension("json.partial");
        let result = (|| -> Result<()> {
            fs::write(&partial, serde_json::to_vec_pretty(self)?)?;
            fs::File::open(&partial)?.sync_all()?;
            fs::rename(&partial, path)?;
            fs::File::open(path.parent().unwrap())?.sync_all()?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&partial);
            let _ = fs::remove_file(path);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(vm_vcpus: u32, vm_memory_mb: u32) -> CheckpointManifest {
        CheckpointManifest {
            format_version: WORKLOAD_CHECKPOINT_FORMAT_VERSION,
            hypervisor: hypervisor::HYPERVISOR_QEMU.to_string(),
            tasks: Vec::new(),
            rootfs: Vec::new(),
            network_macs: Vec::new(),
            vm_vcpus,
            vm_memory_mb,
            vm_state: Vec::new(),
            runtime_fingerprint: "test".to_string(),
            template: None,
        }
    }

    #[test]
    fn published_manifest_preserves_instance_data() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(WORKLOAD_CHECKPOINT_STATE)).unwrap();
        let path = directory.path().join(WORKLOAD_CHECKPOINT_MANIFEST);
        let mut original = manifest(2, 256);
        original.tasks.push(CheckpointTask {
            checkpoint_key: "checkpoint-task".into(),
            task_id: "source-task".into(),
        });

        original.publish(&path).unwrap();
        let loaded = CheckpointManifest::load(directory.path().to_str().unwrap()).unwrap();

        assert_eq!(
            serde_json::to_value(&loaded).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert!(!path.with_extension("json.partial").exists());
    }

    #[test]
    fn manifest_requires_vm_resource_topology_fields() {
        for field in ["vm_vcpus", "vm_memory_mb"] {
            let mut value = serde_json::to_value(manifest(1, 128)).unwrap();
            value.as_object_mut().unwrap().remove(field);
            let error = serde_json::from_value::<CheckpointManifest>(value)
                .err()
                .expect("missing VM topology field must be rejected");
            assert!(error
                .to_string()
                .contains(&format!("missing field `{field}`")));
        }
    }
}
