// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

use anyhow::Result;
use common::error::Error as CommonError;
use serde::{Deserialize, Serialize};

use crate::factory::template::TemplateIdentity;
use crate::sandbox::VirtSandbox;

use super::artifacts::vm;
use super::checkpoint_files;
use hypervisor::VmResourceTopology;

pub(super) const WORKLOAD_CHECKPOINT_FORMAT_VERSION: u32 = 2;
pub(super) const WORKLOAD_CHECKPOINT_MANIFEST: &str = "manifest.json";
pub(super) const WORKLOAD_CHECKPOINT_STATE: &str = "vm";
pub(super) const WORKLOAD_CHECKPOINT_STORAGE: &str = "storage";
pub(super) use super::artifacts::VmCheckpointArtifact as VmStateFile;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointTask {
    pub(super) checkpoint_key: String,
    pub(super) task_id: String,
}

#[derive(Serialize, Deserialize)]
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
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointManifest {
    pub(super) format_version: u32,
    pub(super) hypervisor: String,
    pub(super) tasks: Vec<CheckpointTask>,
    pub(super) rootfs: Vec<CheckpointRootfs>,
    pub(super) network_macs: Vec<[u8; 6]>,
    pub(super) vm_vcpus: u32,
    pub(super) vm_memory_mb: u32,
    pub(super) vm_state: Vec<VmStateFile>,
    pub(super) runtime_fingerprint: String,
    #[serde(default)]
    pub(super) template: Option<TemplateIdentity>,
}

impl VirtSandbox {
    pub(super) async fn load_restore_manifest(
        &self,
        checkpoint_path: &str,
    ) -> Result<CheckpointManifest> {
        self.validate_ckpt_profile().await?;

        let (state_path, manifest_path) = checkpoint_files(checkpoint_path)?;
        let state_metadata = fs::symlink_metadata(&state_path).map_err(|err| {
            CommonError::InvalidSandboxOperation(format!(
                "cannot inspect VM checkpoint directory {}: {err}",
                state_path.display()
            ))
        })?;

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

        Self::validate_task_count(manifest.tasks.len())?;
        Self::validate_network_count(manifest.network_macs.len())?;

        let hypervisor_name = self
            .resource_manager
            .config()
            .await
            .runtime
            .hypervisor_name
            .clone();

        if manifest.format_version != WORKLOAD_CHECKPOINT_FORMAT_VERSION
            || manifest.hypervisor != hypervisor_name
        {
            return Err(CommonError::InvalidSandboxOperation(
                "unsupported workload checkpoint format or hypervisor".to_string(),
            )
            .into());
        }

        Self::ensure_template_identity(
            manifest.template.as_ref(),
            self.template_identity.as_ref(),
        )?;

        Self::validate_vm_topology(manifest.vm_vcpus, manifest.vm_memory_mb)?;
        let native = vm::native_artifacts(&manifest.vm_state)?;

        vm::validate_checkpoint_artifacts(
            &hypervisor_name,
            &native,
            VmResourceTopology {
                vcpus: manifest.vm_vcpus,
                memory_mb: manifest.vm_memory_mb,
            },
        )?;

        Ok(manifest)
    }

    fn validate_vm_topology(vm_vcpus: u32, vm_memory_mb: u32) -> Result<()> {
        if vm_vcpus == 0 || vm_memory_mb == 0 {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint is missing VM resource topology".to_string(),
            )
            .into());
        }

        Ok(())
    }

    fn ensure_template_identity(
        checkpoint: Option<&TemplateIdentity>,
        current: Option<&TemplateIdentity>,
    ) -> Result<()> {
        match (checkpoint, current) {
            (None, None) => Ok(()),
            (Some(checkpoint), Some(current)) if checkpoint == current => Ok(()),
            (Some(_), Some(_)) => Err(CommonError::SandboxOperationPrecondition(
                "checkpoint VM template identity does not match this runtime".to_string(),
            )
            .into()),
            (Some(_), None) => Err(CommonError::SandboxOperationPrecondition(
                "checkpoint requires a VM template but this runtime has none".to_string(),
            )
            .into()),
            (None, Some(_)) => Err(CommonError::SandboxOperationPrecondition(
                "VM template runtime cannot restore a non-template checkpoint".to_string(),
            )
            .into()),
        }
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

    #[test]
    fn manifest_rejects_zero_vm_resource_topology() {
        for (vm_vcpus, vm_memory_mb) in [(0, 128), (1, 0)] {
            let error = VirtSandbox::validate_vm_topology(vm_vcpus, vm_memory_mb)
                .expect_err("zero VM topology must be rejected");
            assert!(error
                .to_string()
                .contains("checkpoint is missing VM resource topology"));
        }
    }

    fn template_identity(memory_sha256: &str) -> TemplateIdentity {
        TemplateIdentity {
            format_version: 1,
            hypervisor: "qemu".to_string(),
            memory_size: 1,
            memory_sha256: memory_sha256.to_string(),
            device_state_size: 2,
            device_state_sha256: "device".to_string(),
            config_sha256: "config".to_string(),
            native_config_sha256: None,
        }
    }

    #[test]
    fn template_identity_match_is_required() {
        let expected = template_identity("expected");
        let different = template_identity("different");
        VirtSandbox::ensure_template_identity(Some(&expected), Some(&expected)).unwrap();
        assert!(
            VirtSandbox::ensure_template_identity(Some(&expected), Some(&different),)
                .unwrap_err()
                .to_string()
                .contains("does not match")
        );
        assert!(
            VirtSandbox::ensure_template_identity(None, Some(&expected),)
                .unwrap_err()
                .to_string()
                .contains("cannot restore a non-template checkpoint")
        );
    }
}
