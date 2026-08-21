// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
        DeviceConfig,
    },
    BlockConfigModern, BlockDeviceAio,
};
use kata_types::mount::{
    DirectVolumeMountInfo, DIRECT_VOLUME_METADATA_FS_GROUP,
    DIRECT_VOLUME_METADATA_FS_GROUP_CHANGE_POLICY,
    KATA_CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4,
};
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{
    direct_volumes::KATA_DIRECT_VOLUME_TYPE,
    utils::{handle_block_volume, is_block_device_readonly},
    Volume,
};

#[derive(Clone)]
pub(crate) struct RawblockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// RawblockVolume for raw block volume
impl RawblockVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        mount_info.validated_confidential_storage()?;
        let blkdev_info = get_block_device_info(d).await;

        // check volume type
        if mount_info.volume_type != KATA_DIRECT_VOLUME_TYPE {
            return Err(anyhow!(
                "volume type {:?} is invalid",
                mount_info.volume_type
            ));
        }

        let fstat = stat::stat(mount_info.device.as_str())
            .with_context(|| format!("stat volume device file: {}", mount_info.device.clone()))?;
        if SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFREG
            && SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFBLK
        {
            return Err(anyhow!(
                "invalid volume device {:?} for volume type {:?}",
                mount_info.device,
                mount_info.volume_type
            ));
        }

        // For a real block device, honor its host read-only flag (BLKROGET) in
        // addition to the mount-derived intent, so a device marked read-only on
        // the host is exposed read-only to the guest. (Not applicable to
        // regular-file backed images.)
        let read_only = read_only
            || (SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK
                && is_block_device_readonly(mount_info.device.as_str()).unwrap_or_else(|e| {
                    warn!(
                        sl!(),
                        "could not query block device read-only flag for {}: {:?}",
                        mount_info.device,
                        e
                    );
                    false
                }));

        let block_config = BlockConfigModern {
            path_on_host: mount_info.device.clone(),
            is_readonly: read_only,
            driver_option: blkdev_info.block_device_driver,
            blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
            num_queues: blkdev_info.num_queues,
            queue_size: blkdev_info.queue_size,
            logical_sector_size: blkdev_info.block_device_logical_sector_size,
            physical_sector_size: blkdev_info.block_device_physical_sector_size,
            ..Default::default()
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfgModern(block_config.clone()))
            .await
            .context("do handle device failed.")?;

        let mut block_volume = handle_block_volume(
            device_info,
            m,
            read_only,
            sid,
            &mount_info.fs_type,
            Some(&mount_info.options),
        )
        .await
        .context("do handle block volume failed")?;

        configure_confidential_storage(&mut block_volume.0, mount_info)?;

        Ok(Self {
            storage: Some(block_volume.0),
            mount: block_volume.1,
            device_id: block_volume.2,
        })
    }
}

fn configure_confidential_storage(
    storage: &mut agent::Storage,
    mount_info: &DirectVolumeMountInfo,
) -> Result<()> {
    let Some(request) = mount_info.validated_confidential_storage()? else {
        return Ok(());
    };

    if request.profile != KATA_CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4 {
        return Err(anyhow!("unsupported confidential storage profile"));
    }

    storage.confidential_storage = Some(agent::ConfidentialStorage {
        profile: agent::ConfidentialStorageProfile::Luks2IntegrityExt4,
        volume_id: request.volume_id.clone(),
        key_uri: request.key_uri.clone(),
    });

    if let Some(group_id) = mount_info.metadata.get(DIRECT_VOLUME_METADATA_FS_GROUP) {
        let group_change_policy = match mount_info
            .metadata
            .get(DIRECT_VOLUME_METADATA_FS_GROUP_CHANGE_POLICY)
            .map(String::as_str)
        {
            Some("OnRootMismatch") => agent::FSGroupChangePolicy::OnRootMismatch,
            _ => agent::FSGroupChangePolicy::Always,
        };
        storage.fs_group = Some(agent::FSGroup {
            group_id: group_id.parse::<u32>()?,
            group_change_policy,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kata_types::mount::{
        ConfidentialStorage, KATA_CONFIDENTIAL_STORAGE_FS_TYPE,
        KATA_CONFIDENTIAL_STORAGE_VOLUME_TYPE,
    };

    use super::*;

    fn confidential_mount_info() -> DirectVolumeMountInfo {
        DirectVolumeMountInfo {
            volume_type: KATA_CONFIDENTIAL_STORAGE_VOLUME_TYPE.to_string(),
            device: "/dev/sda".to_string(),
            fs_type: KATA_CONFIDENTIAL_STORAGE_FS_TYPE.to_string(),
            metadata: HashMap::new(),
            options: Vec::new(),
            confidential_storage: Some(ConfidentialStorage {
                profile: KATA_CONFIDENTIAL_STORAGE_PROFILE_LUKS2_INTEGRITY_EXT4.to_string(),
                volume_id: "tenant/workload/volume".to_string(),
                key_uri: "kbs:///tenant/storage/key".to_string(),
            }),
        }
    }

    #[test]
    fn configure_confidential_storage_request() {
        let mut mount_info = confidential_mount_info();
        mount_info
            .metadata
            .insert("fsGroup".to_string(), "3000".to_string());
        mount_info.metadata.insert(
            "fsGroupChangePolicy".to_string(),
            "OnRootMismatch".to_string(),
        );
        let mut storage = agent::Storage::default();

        configure_confidential_storage(&mut storage, &mount_info).unwrap();

        assert_eq!(
            storage.confidential_storage,
            Some(agent::ConfidentialStorage {
                profile: agent::ConfidentialStorageProfile::Luks2IntegrityExt4,
                volume_id: "tenant/workload/volume".to_string(),
                key_uri: "kbs:///tenant/storage/key".to_string(),
            })
        );
        assert_eq!(
            storage.fs_group,
            Some(agent::FSGroup {
                group_id: 3000,
                group_change_policy: agent::FSGroupChangePolicy::OnRootMismatch,
            })
        );
    }

    #[test]
    fn configure_confidential_storage_rejects_downgrade() {
        let mut mount_info = confidential_mount_info();
        mount_info.fs_type = "ext4".to_string();
        let mut storage = agent::Storage::default();

        assert!(configure_confidential_storage(&mut storage, &mount_info).is_err());
        assert!(storage.confidential_storage.is_none());
    }
}

#[async_trait]
impl Volume for RawblockVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        let s = if let Some(s) = self.storage.as_ref() {
            vec![s.clone()]
        } else {
            vec![]
        };

        Ok(s)
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        device_manager
            .write()
            .await
            .try_remove_device(&self.device_id)
            .await
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }
}
