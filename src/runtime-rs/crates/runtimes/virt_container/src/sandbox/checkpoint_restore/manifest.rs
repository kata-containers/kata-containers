// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use crate::factory::template::TemplateIdentity;
use serde::{Deserialize, Serialize};
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
}
