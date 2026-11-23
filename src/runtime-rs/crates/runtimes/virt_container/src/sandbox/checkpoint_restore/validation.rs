// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

use anyhow::{Context, Result};
use common::error::Error as CommonError;
use common::types::{CheckpointSandboxRequest, RestoreSandboxRequest, RestoredSandboxTask};
use hypervisor::device::DeviceType;
use hypervisor::{
    self, BlockDeviceModern, VmResourceTopology, KATA_BLK_DEV_TYPE, VIRTIO_BLOCK_MMIO,
    VIRTIO_BLOCK_PCI,
};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use resource::ResourceManager;

use crate::factory::template::TemplateIdentity;
use crate::sandbox::StartMode;

use super::artifacts::{vm, IMMUTABLE_ROOTFS_OPTION};
use super::identity::RUNTIME_ASSET_IDENTITY_OPTION;
use super::manifest::{CheckpointManifest, WORKLOAD_CHECKPOINT_FORMAT_VERSION};
use super::PendingRestore;

/// Check runtime configuration before checkpoint or restore work begins.
pub(super) struct RuntimeValidation<'a> {
    pub(super) resources: &'a ResourceManager,
}

impl RuntimeValidation<'_> {
    fn validate_config(hypervisor_name: &str, config: &HypervisorConfig) -> Result<()> {
        if config.security_info.confidential_guest {
            return Err(CommonError::SandboxOperationPrecondition(
                "workload checkpoint/restore does not support confidential guests".to_string(),
            )
            .into());
        }

        // FIXME: The shared_fs check is a temporary workaround for.
        // In future, we should support shared_fs=virtio-fs for checkpoint/restore.
        if !matches!(config.shared_fs.shared_fs.as_deref(), None | Some("none")) {
            return Err(CommonError::SandboxOperationPrecondition(
                "workload checkpoint/restore requires shared_fs=none".to_string(),
            )
            .into());
        }

        match hypervisor_name {
            hypervisor::HYPERVISOR_QEMU => {
                if config.blockdev_info.block_device_driver != VIRTIO_BLOCK_PCI {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "QEMU checkpoint/restore requires block_device_driver=virtio-blk-pci"
                            .to_string(),
                    )
                    .into());
                }
            }
            hypervisor::HYPERVISOR_NAME_CH => {
                if config.blockdev_info.block_device_driver != VIRTIO_BLOCK_PCI {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "Cloud Hypervisor checkpoint/restore requires block_device_driver=virtio-blk-pci"
                            .to_string(),
                    )
                    .into());
                }
            }
            hypervisor::HYPERVISOR_DRAGONBALL => {
                if config.blockdev_info.block_device_driver != VIRTIO_BLOCK_MMIO
                    || config.boot_info.vm_rootfs_driver != VIRTIO_BLOCK_MMIO
                {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "Dragonball checkpoint/restore requires block_device_driver and vm_rootfs_driver to use virtio-blk-mmio"
                            .to_string(),
                    )
                    .into());
                }
                if !config.network_info.disable_vhost_net {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "Dragonball checkpoint/restore requires disable_vhost_net=true".to_string(),
                    )
                    .into());
                }
                if !config.machine_info.entropy_source.is_empty() {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "Dragonball checkpoint/restore does not support a virtio-rng device"
                            .to_string(),
                    )
                    .into());
                }
                // FIXME: The reclaim_guest_freed_memory check is a temporary workaround for Dragonball checkpoint/restore.
                // In future, we should support reclaim_guest_freed_memory for Dragonball checkpoint/restore.
                if config.device_info.reclaim_guest_freed_memory {
                    return Err(CommonError::SandboxOperationPrecondition(
                        "Dragonball checkpoint/restore does not support a balloon device"
                            .to_string(),
                    )
                    .into());
                }
            }
            other => {
                return Err(CommonError::SandboxOperationUnsupported(format!(
                    "workload checkpoint/restore is unsupported by hypervisor {other}"
                ))
                .into())
            }
        }

        Ok(())
    }
}

impl RuntimeValidation<'_> {
    pub(super) async fn validate(&self) -> Result<()> {
        let config = self.resources.config().await;
        if config.runtime.passfd_listener_port != 0 {
            return Err(CommonError::SandboxOperationPrecondition(
                "workload checkpoint/restore requires passfd_listener_port=0".to_string(),
            )
            .into());
        }

        let hypervisor_name = config.runtime.hypervisor_name.as_str();
        let hypervisor_config = config.hypervisor.get(hypervisor_name).ok_or_else(|| {
            CommonError::SandboxOperationPrecondition(format!(
                "missing configuration for hypervisor {hypervisor_name}"
            ))
        })?;
        Self::validate_config(hypervisor_name, hypervisor_config)?;

        Ok(())
    }
}

/// Check tasks, runtime profile, sandbox identity, options, and output directory.
pub(super) struct CheckpointRequestValidation<'a> {
    pub(super) sandbox_id: &'a str,
    pub(super) request: &'a CheckpointSandboxRequest,
    pub(super) resources: &'a ResourceManager,
}

impl CheckpointRequestValidation<'_> {
    pub(super) async fn validate(&self) -> Result<()> {
        let req = self.request;
        validate_task_count(req.tasks.len())?;
        RuntimeValidation {
            resources: self.resources,
        }
        .validate()
        .await?;

        if req.sandbox_id != self.sandbox_id {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint sandbox id does not match runtime sandbox".to_string(),
            )
            .into());
        }

        if req
            .options
            .keys()
            .any(|key| !checkpoint_option_supported(key))
        {
            return Err(CommonError::InvalidSandboxOperation(
                "unsupported checkpoint option".to_string(),
            )
            .into());
        }

        // validate checkpoint output_path
        super::checkpoint_files(&req.output_path)?;
        if fs::read_dir(&req.output_path)
            .context("read checkpoint output directory")?
            .next()
            .is_some()
        {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint output directory is not empty".to_string(),
            )
            .into());
        }

        Ok(())
    }
}

/// Read live devices, check rootfs ownership and networks, and return their MACs.
/// Run again after freezing tasks; the earlier inventory must not be reused.
pub(super) struct CheckpointInventoryValidation<'a> {
    pub(super) request: &'a CheckpointSandboxRequest,
    pub(super) resources: &'a ResourceManager,
}

impl CheckpointInventoryValidation<'_> {
    fn validate_blocks(
        req: &CheckpointSandboxRequest,
        blocks: &HashMap<String, BlockDeviceModern>,
    ) -> Result<()> {
        let mut rootfs_ids = HashSet::with_capacity(req.tasks.len());
        for task in &req.tasks {
            if task.rootfs_device_ids.is_empty() {
                return Err(CommonError::SandboxOperationPrecondition(format!(
                    "checkpoint task {} has no resolved rootfs device",
                    task.task_id
                ))
                .into());
            }

            for rootfs_device_id in &task.rootfs_device_ids {
                if !rootfs_ids.insert(rootfs_device_id.clone()) {
                    return Err(CommonError::SandboxOperationPrecondition(format!(
                        "checkpoint rootfs device {rootfs_device_id} is used by multiple tasks"
                    ))
                    .into());
                }

                if !blocks.contains_key(rootfs_device_id) {
                    return Err(CommonError::SandboxOperationPrecondition(format!(
                        "checkpoint task {} rootfs device {} is absent",
                        task.task_id, rootfs_device_id
                    ))
                    .into());
                }
            }
        }

        let external_blocks = blocks
            .keys()
            .filter(|device_id| !rootfs_ids.contains(*device_id))
            .count();

        if external_blocks != 0 {
            return Err(CommonError::SandboxOperationUnsupported(
                "external block volumes require a stable storage snapshot identity".to_string(),
            )
            .into());
        }

        Ok(())
    }
}

impl CheckpointInventoryValidation<'_> {
    pub(super) async fn validate(&self) -> Result<Vec<[u8; 6]>> {
        let req = self.request;
        let device_manager = self.resources.get_device_manager().await;
        let device_manager = device_manager.read().await;
        let config = self.resources.config().await;
        let hypervisor_name = config.runtime.hypervisor_name.as_str();
        let hypervisor_config = config.hypervisor.get(hypervisor_name).ok_or_else(|| {
            CommonError::SandboxOperationPrecondition(format!(
                "missing configuration for hypervisor {hypervisor_name}"
            ))
        })?;
        let devices = device_manager.checkpoint_devices().await;

        validate_checkpoint_device_inventory(
            &devices,
            hypervisor_name,
            &hypervisor_config.boot_info.initrd,
        )
        .await?;

        let blocks = device_manager
            .checkpoint_block_devices()
            .await
            .into_iter()
            .map(|block| (block.device_id.clone(), block))
            .collect::<HashMap<_, _>>();

        Self::validate_blocks(req, &blocks)?;

        let network_macs = device_manager.checkpoint_network_macs().await;
        validate_network_count(network_macs.len())?;

        Ok(network_macs)
    }
}

/// Check manifest counts, format, hypervisor, template, topology, and native inventory.
pub(super) struct RestoreManifestValidation<'a> {
    pub(super) manifest: &'a CheckpointManifest,
    pub(super) resources: &'a ResourceManager,
    pub(super) template: Option<&'a TemplateIdentity>,
}

impl RestoreManifestValidation<'_> {
    fn validate_topology(vm_vcpus: u32, vm_memory_mb: u32) -> Result<()> {
        if vm_vcpus == 0 || vm_memory_mb == 0 {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint is missing VM resource topology".to_string(),
            )
            .into());
        }

        Ok(())
    }

    fn validate_template_identity(
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

impl RestoreManifestValidation<'_> {
    pub(super) async fn validate(&self) -> Result<()> {
        let manifest = self.manifest;
        validate_task_count(manifest.tasks.len())?;
        validate_network_count(manifest.network_macs.len())?;

        let hypervisor_name = self
            .resources
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

        Self::validate_template_identity(manifest.template.as_ref(), self.template)?;

        Self::validate_topology(manifest.vm_vcpus, manifest.vm_memory_mb)?;
        let native = vm::VmCheckpointArtifact::from_manifest(&manifest.vm_state)?;

        vm::VmArtifacts {
            hypervisor_name: &hypervisor_name,
            artifacts: native,
            topology: VmResourceTopology {
                vcpus: manifest.vm_vcpus,
                memory_mb: manifest.vm_memory_mb,
            },
        }
        .validate()
        .await?;

        Ok(())
    }
}

/// Check registered devices before starting a restored VM; skip normal startup.
pub(in crate::sandbox) struct RestoreInventoryValidation<'a> {
    pub(in crate::sandbox) mode: StartMode,
    pub(in crate::sandbox) resources: &'a ResourceManager,
}

impl RestoreInventoryValidation<'_> {
    pub(in crate::sandbox) async fn validate(&self) -> Result<()> {
        if !matches!(self.mode, StartMode::Restore) {
            // No restore device inventory validation is required if the sandbox is not restoring from a checkpoint.
            return Ok(());
        }

        let device_manager = self.resources.get_device_manager().await;
        let device_manager = device_manager.read().await;
        let config = self.resources.config().await;
        let hypervisor_name = config.runtime.hypervisor_name.as_str();
        let hypervisor_config = config.hypervisor.get(hypervisor_name).ok_or_else(|| {
            CommonError::SandboxOperationPrecondition(format!(
                "missing configuration for hypervisor {hypervisor_name}"
            ))
        })?;
        let devices = device_manager.checkpoint_devices().await;

        validate_checkpoint_device_inventory(
            &devices,
            hypervisor_name,
            &hypervisor_config.boot_info.initrd,
        )
        .await
    }
}

/// Check the prepared request and task/rootfs mappings before guest rebinding.
/// The caller owns taking pending state and performing the Agent RPC.
pub(super) struct RestoreBindingValidation<'a> {
    pub(super) request: &'a RestoreSandboxRequest,
    pub(super) pending: &'a PendingRestore,
}

impl RestoreBindingValidation<'_> {
    fn map_tasks(&self) -> Result<(Vec<agent::ContainerIDMapping>, Vec<RestoredSandboxTask>)> {
        let req = self.request;
        let manifest = &self.pending.manifest;
        if manifest.tasks.len() != req.tasks.len() || req.tasks.is_empty() {
            return Err(CommonError::InvalidSandboxOperation(
                "restore task inventory does not match checkpoint".to_string(),
            )
            .into());
        }

        let source_tasks = manifest
            .tasks
            .iter()
            .map(|task| (task.checkpoint_key.as_str(), task.task_id.as_str()))
            .collect::<HashMap<_, _>>();

        if source_tasks.len() != manifest.tasks.len() {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint contains duplicate task keys".to_string(),
            )
            .into());
        }

        let source_rootfs = manifest
            .rootfs
            .iter()
            .map(|rootfs| (rootfs.checkpoint_key.as_str(), rootfs))
            .collect::<HashMap<_, _>>();

        if source_rootfs.len() != manifest.rootfs.len()
            || source_rootfs.len() != source_tasks.len()
            || source_rootfs.values().any(|rootfs| {
                rootfs.device_id.is_empty()
                    || rootfs
                        .immutable_lower
                        .as_ref()
                        .is_some_and(|lower| lower.device_id.is_empty())
            })
        {
            return Err(CommonError::InvalidSandboxOperation(
                "checkpoint contains invalid rootfs inventory".to_string(),
            )
            .into());
        }

        let task_count = req.tasks.len();
        let mut mappings = Vec::with_capacity(task_count);
        let mut tasks = Vec::with_capacity(task_count);
        let mut restore_keys = HashSet::with_capacity(task_count);
        let mut new_ids = HashSet::with_capacity(task_count);

        for task in &req.tasks {
            if task.checkpoint_key.is_empty() || !restore_keys.insert(task.checkpoint_key.as_str())
            {
                return Err(CommonError::InvalidSandboxOperation(
                    "invalid or duplicate restore checkpoint key".to_string(),
                )
                .into());
            }

            let old_id = source_tasks
                .get(task.checkpoint_key.as_str())
                .ok_or_else(|| {
                    CommonError::InvalidSandboxOperation(format!(
                        "restore task key {} is absent",
                        task.checkpoint_key
                    ))
                })?;

            let rootfs = source_rootfs
                .get(task.checkpoint_key.as_str())
                .ok_or_else(|| {
                    CommonError::InvalidSandboxOperation(format!(
                        "restore task key {} has no rootfs",
                        task.checkpoint_key
                    ))
                })?;

            if task.task_id.is_empty() || !new_ids.insert(task.task_id.as_str()) {
                return Err(CommonError::InvalidSandboxOperation(format!(
                    "invalid or duplicate restored task id {}",
                    task.task_id
                ))
                .into());
            }

            let mut rootfs_device_ids =
                Vec::with_capacity(1 + usize::from(rootfs.immutable_lower.is_some()));
            rootfs_device_ids.push(rootfs.device_id.clone());
            if let Some(lower) = &rootfs.immutable_lower {
                rootfs_device_ids.push(lower.device_id.clone());
            }

            // Process and resource config is already in the checkpointed guest.
            mappings.push(agent::ContainerIDMapping {
                old_id: (*old_id).to_string(),
                new_id: task.task_id.clone(),
            });

            tasks.push(RestoredSandboxTask {
                checkpoint_key: task.checkpoint_key.clone(),
                task_id: task.task_id.clone(),
                rootfs_device_ids,
            });
        }

        Ok((mappings, tasks))
    }
}

impl RestoreBindingValidation<'_> {
    pub(super) async fn validate(
        &self,
    ) -> Result<(Vec<agent::ContainerIDMapping>, Vec<RestoredSandboxTask>)> {
        let req = self.request;
        let pending = self.pending;

        if pending.checkpoint_path != req.checkpoint_path || pending.options != req.options {
            return Err(CommonError::InvalidSandboxOperation(
                "restore request does not match prepared checkpoint".to_string(),
            )
            .into());
        }

        if pending.boot.is_some() {
            return Err(CommonError::SandboxOperationPrecondition(
                "restored VM has not consumed its prepared boot state".to_string(),
            )
            .into());
        }

        self.map_tasks()
    }
}

async fn validate_checkpoint_device_inventory(
    devices: &[DeviceType],
    hypervisor_name: &str,
    initrd: &str,
) -> Result<()> {
    // Preserve the existing QEMU device allowlist. Cloud Hypervisor and
    // Dragonball did not previously impose a VMM-side device allowlist.
    if hypervisor_name != hypervisor::HYPERVISOR_QEMU {
        return Ok(());
    }

    for device in devices {
        match device {
            DeviceType::Vsock(_)
            | DeviceType::HybridVsock(_)
            | DeviceType::PortDevice(_)
            | DeviceType::Network(_) => {}
            DeviceType::BlockModern(block) => {
                let block = block.lock().await;
                if block.config.path_on_host == initrd {
                    continue;
                }

                let path = Path::new(&block.config.path_on_host);
                let valid_backing = fs::symlink_metadata(path)
                    .map(|metadata| {
                        !metadata.file_type().is_symlink()
                            && (metadata.is_file() || metadata.file_type().is_block_device())
                    })
                    .unwrap_or(false);

                if !path.is_absolute()
                    || block.config.driver_option != KATA_BLK_DEV_TYPE
                    || !valid_backing
                {
                    return Err(CommonError::SandboxOperationUnsupported(format!(
                        "workload checkpoint/restore does not support external device {device}"
                    ))
                    .into());
                }
            }
            _ => {
                return Err(CommonError::SandboxOperationUnsupported(format!(
                    "workload checkpoint/restore does not support external device {device}"
                ))
                .into())
            }
        }
    }

    Ok(())
}

fn validate_task_count(task_count: usize) -> Result<()> {
    if task_count == 0 {
        return Err(CommonError::SandboxOperationPrecondition(
            "workload checkpoint/restore requires at least one task".to_string(),
        )
        .into());
    }

    Ok(())
}

fn validate_network_count(count: usize) -> Result<()> {
    if count > 1 {
        return Err(CommonError::SandboxOperationUnsupported(
            "multiple network interfaces require stable checkpoint-to-CNI interface mapping"
                .to_string(),
        )
        .into());
    }

    Ok(())
}

fn checkpoint_option_supported(key: &str) -> bool {
    key == IMMUTABLE_ROOTFS_OPTION || key == RUNTIME_ASSET_IDENTITY_OPTION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_zero_vm_resource_topology() {
        for (vm_vcpus, vm_memory_mb) in [(0, 128), (1, 0)] {
            let error = RestoreManifestValidation::validate_topology(vm_vcpus, vm_memory_mb)
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
        RestoreManifestValidation::validate_template_identity(Some(&expected), Some(&expected))
            .unwrap();
        assert!(RestoreManifestValidation::validate_template_identity(
            Some(&expected),
            Some(&different),
        )
        .unwrap_err()
        .to_string()
        .contains("does not match"));
        assert!(
            RestoreManifestValidation::validate_template_identity(None, Some(&expected),)
                .unwrap_err()
                .to_string()
                .contains("cannot restore a non-template checkpoint")
        );
    }

    #[test]
    fn checkpoint_runtime_profile_is_validated() {
        let mut qemu = HypervisorConfig::default();
        qemu.blockdev_info.block_device_driver = VIRTIO_BLOCK_PCI.to_string();
        RuntimeValidation::validate_config(hypervisor::HYPERVISOR_QEMU, &qemu)
            .expect("valid QEMU checkpoint profile");
        qemu.shared_fs.shared_fs = Some("virtio-fs".to_string());
        assert!(RuntimeValidation::validate_config(hypervisor::HYPERVISOR_QEMU, &qemu).is_err());

        let mut dragonball = HypervisorConfig::default();
        dragonball.blockdev_info.block_device_driver = VIRTIO_BLOCK_MMIO.to_string();
        dragonball.boot_info.vm_rootfs_driver = VIRTIO_BLOCK_MMIO.to_string();
        dragonball.network_info.disable_vhost_net = true;
        dragonball.machine_info.entropy_source.clear();
        dragonball.device_info.reclaim_guest_freed_memory = false;
        RuntimeValidation::validate_config(hypervisor::HYPERVISOR_DRAGONBALL, &dragonball)
            .expect("valid Dragonball checkpoint profile");
        dragonball.machine_info.entropy_source = "/dev/urandom".to_string();
        assert!(
            RuntimeValidation::validate_config(hypervisor::HYPERVISOR_DRAGONBALL, &dragonball,)
                .is_err()
        );

        assert!(RuntimeValidation::validate_config("unsupported-vmm", &qemu).is_err());
    }

    #[test]
    fn checkpoint_options_remain_allowlisted() {
        assert!(checkpoint_option_supported(IMMUTABLE_ROOTFS_OPTION));
        assert!(checkpoint_option_supported(RUNTIME_ASSET_IDENTITY_OPTION));
        assert!(!checkpoint_option_supported("unknown"));
    }
    fn restore_inventory() -> (RestoreSandboxRequest, CheckpointManifest) {
        let rootfs = |key: &str| {
            serde_json::json!({
                "checkpoint_key": key, "file": format!("storage/{key}.sparse"),
                "format": "kata-sparse-raw-v1", "device_id": format!("dev-{key}"),
                "index": 1, "driver_option": "virtio-blk", "virt_path": "/dev/vdb",
                "is_direct": null, "discard_unmap": false, "aio": "threads",
                "queue_size": 128, "num_queues": 1, "logical_sector_size": 512,
                "physical_sector_size": 512, "serial_override": "",
                "logical_size": 4096, "encoded_size": 32, "sha256": "a".repeat(64)
            })
        };
        let mut a = rootfs("a");
        a["immutable_lower"] = serde_json::json!({
            "device_id": "lower-a", "index": 2, "driver_option": "virtio-blk",
            "virt_path": "/dev/vdc", "is_direct": null, "discard_unmap": false,
            "aio": "threads", "queue_size": 128, "num_queues": 1,
            "logical_sector_size": 512, "physical_sector_size": 512,
            "serial_override": "", "size": 4096, "sha256": "b".repeat(64)
        });
        let manifest = serde_json::from_value(serde_json::json!({
            "format_version": 16, "hypervisor": "qemu",
            "tasks": [
                {"checkpoint_key": "a", "task_id": "old-a"},
                {"checkpoint_key": "b", "task_id": "old-b"}
            ],
            "rootfs": [a, rootfs("b")], "network_macs": [],
            "vm_vcpus": 1, "vm_memory_mb": 128, "vm_state": [],
            "runtime_fingerprint": "test"
        }))
        .unwrap();
        let request = RestoreSandboxRequest {
            sandbox_config: common::types::SandboxConfig {
                sandbox_id: "inventory-test".into(),
                hostname: "inventory-test".into(),
                dns: Vec::new(),
                network_env: common::SandboxNetworkEnv::default(),
                annotations: HashMap::new(),
                hooks: None,
                state: runtime_spec::State {
                    version: Default::default(),
                    id: String::new(),
                    status: runtime_spec::ContainerState::Creating,
                    pid: 0,
                    bundle: String::new(),
                    annotations: Default::default(),
                },
                shm_size: 0,
                config_path: None,
            },
            checkpoint_path: "/unused-inventory-test".into(),
            options: HashMap::new(),
            tasks: ["b", "a"]
                .iter()
                .map(|&key| common::types::SandboxRestoreTask {
                    checkpoint_key: key.into(),
                    task_id: format!("new-{key}"),
                })
                .collect(),
            operation_timeout: None,
        };
        (request, manifest)
    }

    #[tokio::test]
    async fn restore_mapping_preserves_task_order() {
        let (request, manifest) = restore_inventory();
        let pending = pending_restore(&request, manifest);
        let (mappings, tasks) = RestoreBindingValidation {
            request: &request,
            pending: &pending,
        }
        .validate()
        .await
        .unwrap();
        assert_eq!(
            mappings
                .iter()
                .map(|mapping| (mapping.old_id.as_str(), mapping.new_id.as_str()))
                .collect::<Vec<_>>(),
            [("old-b", "new-b"), ("old-a", "new-a")]
        );
        assert_eq!(
            tasks
                .iter()
                .map(|task| (task.checkpoint_key.as_str(), task.task_id.as_str()))
                .collect::<Vec<_>>(),
            [("b", "new-b"), ("a", "new-a")]
        );
        assert_eq!(tasks[0].rootfs_device_ids, ["dev-b"]);
        assert_eq!(tasks[1].rootfs_device_ids, ["dev-a", "lower-a"]);
    }

    #[tokio::test]
    async fn restore_mapping_preserves_error_order() {
        for case in 0..16 {
            let (mut request, mut manifest) = restore_inventory();
            let expected = match case {
                0 => {
                    request.tasks.pop();
                    "restore task inventory does not match checkpoint"
                }
                1 => {
                    request.tasks.clear();
                    manifest.tasks.clear();
                    manifest.rootfs.clear();
                    "restore task inventory does not match checkpoint"
                }
                2 => {
                    manifest.tasks[1].checkpoint_key = "a".into();
                    "checkpoint contains duplicate task keys"
                }
                3 => {
                    manifest.rootfs.pop();
                    "checkpoint contains invalid rootfs inventory"
                }
                4 => {
                    manifest.rootfs[1].checkpoint_key = "a".into();
                    "checkpoint contains invalid rootfs inventory"
                }
                5 => {
                    manifest.rootfs[0].device_id.clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                6 => {
                    manifest.rootfs[0]
                        .immutable_lower
                        .as_mut()
                        .unwrap()
                        .device_id
                        .clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                7 => {
                    request.tasks[0].checkpoint_key = "missing".into();
                    "restore task key missing is absent"
                }
                8 => {
                    manifest.rootfs[0].checkpoint_key = "missing".into();
                    "restore task key a has no rootfs"
                }
                9 => {
                    request.tasks[0].task_id.clear();
                    "invalid or duplicate restored task id"
                }
                10 => {
                    request.tasks[1].task_id = request.tasks[0].task_id.clone();
                    "invalid or duplicate restored task id"
                }
                11 => {
                    manifest.rootfs[0].device_id.clear();
                    request.tasks[0].checkpoint_key = "missing".into();
                    "checkpoint contains invalid rootfs inventory"
                }
                12 => {
                    request.tasks[0].checkpoint_key = "missing".into();
                    request.tasks[0].task_id.clear();
                    "restore task key missing is absent"
                }
                13 => {
                    manifest.rootfs[1].checkpoint_key = "a".into();
                    manifest.rootfs[0].device_id.clear();
                    "checkpoint contains invalid rootfs inventory"
                }
                14 => {
                    request.tasks.pop();
                    manifest.tasks[1].checkpoint_key = "a".into();
                    "restore task inventory does not match checkpoint"
                }
                15 => {
                    request.tasks[1].checkpoint_key = request.tasks[0].checkpoint_key.clone();
                    "invalid or duplicate restore checkpoint key"
                }
                _ => unreachable!(),
            };
            let pending = pending_restore(&request, manifest);
            let error = RestoreBindingValidation {
                request: &request,
                pending: &pending,
            }
            .validate()
            .await
            .err()
            .expect("invalid inventory must be rejected");
            assert!(matches!(
                error.downcast_ref::<CommonError>(),
                Some(CommonError::InvalidSandboxOperation(_))
            ));
            assert!(
                error.to_string().contains(expected),
                "case {}: {}",
                case,
                error
            );
        }
    }

    fn pending_restore(
        request: &RestoreSandboxRequest,
        manifest: CheckpointManifest,
    ) -> PendingRestore {
        PendingRestore {
            boot: None,
            checkpoint_path: request.checkpoint_path.clone(),
            options: request.options.clone(),
            manifest,
        }
    }

    #[tokio::test]
    async fn restore_binding_checks_prepared_request_before_task_inventory() {
        for mismatched_options in [false, true] {
            let (mut request, manifest) = restore_inventory();
            let pending = pending_restore(&request, manifest);
            if mismatched_options {
                request.options.insert("changed".into(), "true".into());
            } else {
                request.checkpoint_path.push_str("-changed");
            }
            request.tasks.clear();
            let error = RestoreBindingValidation {
                request: &request,
                pending: &pending,
            }
            .validate()
            .await
            .err()
            .expect("request mismatch must be rejected first");
            assert!(matches!(
                error.downcast_ref::<CommonError>(),
                Some(CommonError::InvalidSandboxOperation(message))
                    if message == "restore request does not match prepared checkpoint"
            ));
        }
    }
}
