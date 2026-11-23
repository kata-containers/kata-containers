// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

use anyhow::{Context, Result};
use common::error::Error as CommonError;
use common::types::CheckpointSandboxRequest;
use hypervisor::device::DeviceType;
use hypervisor::{self, BlockDeviceModern, KATA_BLK_DEV_TYPE, VIRTIO_BLOCK_PCI};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;

use crate::sandbox::VirtSandbox;

impl VirtSandbox {
    fn validate_ckpt_profile_config(
        hypervisor_name: &str,
        config: &HypervisorConfig,
    ) -> Result<()> {
        if config.security_info.confidential_guest {
            return Err(CommonError::SandboxOperationPrecondition(
                "workload checkpoint/restore does not support confidential guests".to_string(),
            )
            .into());
        }

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

            other => {
                return Err(CommonError::SandboxOperationUnsupported(format!(
                    "workload checkpoint/restore is unsupported by hypervisor {other}"
                ))
                .into())
            }
        }

        Ok(())
    }

    pub(super) async fn validate_ckpt_profile(&self) -> Result<()> {
        let config = self.resource_manager.config().await;
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
        Self::validate_ckpt_profile_config(hypervisor_name, hypervisor_config)?;

        Ok(())
    }

    async fn validate_checkpoint_device_inventory(
        devices: &[DeviceType],
        hypervisor_name: &str,
        initrd: &str,
    ) -> Result<()> {
        // Preserve the existing QEMU device allowlist.
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

    pub(super) fn validate_task_count(task_count: usize) -> Result<()> {
        if task_count == 0 {
            return Err(CommonError::SandboxOperationPrecondition(
                "workload checkpoint/restore requires at least one task".to_string(),
            )
            .into());
        }

        Ok(())
    }

    pub(super) fn validate_network_count(count: usize) -> Result<()> {
        if count > 1 {
            return Err(CommonError::SandboxOperationUnsupported(
                "multiple network interfaces require stable checkpoint-to-CNI interface mapping"
                    .to_string(),
            )
            .into());
        }

        Ok(())
    }

    pub(super) fn validate_checkpoint_blocks(
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

    pub(super) async fn validate_checkpoint_devices(
        &self,
        req: &CheckpointSandboxRequest,
    ) -> Result<Vec<[u8; 6]>> {
        let device_manager = self.resource_manager.get_device_manager().await;
        let device_manager = device_manager.read().await;
        let config = self.resource_manager.config().await;
        let hypervisor_name = config.runtime.hypervisor_name.as_str();
        let hypervisor_config = config.hypervisor.get(hypervisor_name).ok_or_else(|| {
            CommonError::SandboxOperationPrecondition(format!(
                "missing configuration for hypervisor {hypervisor_name}"
            ))
        })?;
        let devices = device_manager.checkpoint_devices().await;

        Self::validate_checkpoint_device_inventory(
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

        Self::validate_checkpoint_blocks(req, &blocks)?;

        let network_macs = device_manager.checkpoint_network_macs().await;
        Self::validate_network_count(network_macs.len())?;

        Ok(network_macs)
    }

    pub(super) async fn validate_checkpoint_request(
        &self,
        req: &CheckpointSandboxRequest,
    ) -> Result<()> {
        Self::validate_task_count(req.tasks.len())?;
        self.validate_ckpt_profile().await?;

        if req.sandbox_id != self.sid {
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

fn checkpoint_option_supported(_key: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_profile_validation_is_owned_by_the_sandbox() {
        let mut qemu = HypervisorConfig::default();
        qemu.blockdev_info.block_device_driver = VIRTIO_BLOCK_PCI.to_string();
        VirtSandbox::validate_ckpt_profile_config(hypervisor::HYPERVISOR_QEMU, &qemu)
            .expect("valid QEMU checkpoint profile");
        qemu.shared_fs.shared_fs = Some("virtio-fs".to_string());
        assert!(
            VirtSandbox::validate_ckpt_profile_config(hypervisor::HYPERVISOR_QEMU, &qemu).is_err()
        );

        assert!(VirtSandbox::validate_ckpt_profile_config("unsupported-vmm", &qemu).is_err());
    }

    #[test]
    fn checkpoint_options_remain_allowlisted() {
        assert!(!checkpoint_option_supported("unknown"));
    }
}
