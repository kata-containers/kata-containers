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
        driver::virtio_blk_modern::BlockSourceFormat,
        DeviceConfig,
    },
    BlockConfigModern, BlockDeviceAio,
};
use kata_types::{
    mount::DirectVolumeMountInfo,
    qcow2::{parse_qcow2_volume, DiscardMode, Qcow2Volume},
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
        discard_supported: bool,
    ) -> Result<Self> {
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

        // The CSI declares the format; it is never probed from the image. A
        // qcow2 volume is a top image, optionally layered on read-only backing
        // images that Kata only attaches and never creates. The hypervisor
        // checks each image when it opens it.
        let qcow2 = parse_qcow2_volume(&mount_info.metadata, &mount_info.device)
            .context("invalid qcow2 direct volume metadata")?;

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

        let discard_unmap = qcow2_discard_unmap(qcow2.as_ref(), read_only, discard_supported);
        let source = qcow2.map_or(BlockSourceFormat::Raw, BlockSourceFormat::Qcow2);

        let block_config = BlockConfigModern {
            path_on_host: mount_info.device.clone(),
            is_readonly: read_only,
            source,
            discard_unmap,
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

        let block_volume = handle_block_volume(
            device_info,
            m,
            read_only,
            sid,
            &mount_info.fs_type,
            Some(&mount_info.options),
        )
        .await
        .context("do handle block volume failed")?;

        Ok(Self {
            storage: Some(block_volume.0),
            mount: block_volume.1,
            device_id: block_volume.2,
        })
    }
}

/// Whether QEMU should free the space of clusters the guest discards.
///
/// Only a writable qcow2 volume whose CSI set the discard key to "unmap" gets
/// it; QEMU's own default is to ignore discards. A read-only volume cannot
/// free anything, and raw direct volumes keep their behaviour of never passing
/// discards to the host.
fn qcow2_discard_unmap(
    qcow2: Option<&Qcow2Volume>,
    read_only: bool,
    discard_supported: bool,
) -> bool {
    discard_supported
        && !read_only
        && matches!(qcow2, Some(chain) if chain.discard == DiscardMode::Unmap)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_writable_qcow2_volumes_asking_for_it_unmap_discards() {
        let chain = |discard| Qcow2Volume {
            discard,
            ..Default::default()
        };
        let unmap = chain(DiscardMode::Unmap);
        let ignore = chain(DiscardMode::Ignore);

        struct TestData<'a> {
            name: &'a str,
            qcow2: Option<&'a Qcow2Volume>,
            read_only: bool,
            discard_supported: bool,
            unmap: bool,
        }
        let tests = [
            TestData {
                name: "unmap",
                qcow2: Some(&unmap),
                read_only: false,
                discard_supported: true,
                unmap: true,
            },
            TestData {
                name: "ignore",
                qcow2: Some(&ignore),
                read_only: false,
                discard_supported: true,
                unmap: false,
            },
            TestData {
                name: "read-only volume",
                qcow2: Some(&unmap),
                read_only: true,
                discard_supported: true,
                unmap: false,
            },
            TestData {
                name: "hypervisor without discard support",
                qcow2: Some(&unmap),
                read_only: false,
                discard_supported: false,
                unmap: false,
            },
            TestData {
                name: "raw volume",
                qcow2: None,
                read_only: false,
                discard_supported: true,
                unmap: false,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            assert_eq!(
                qcow2_discard_unmap(d.qcow2, d.read_only, d.discard_supported),
                d.unmap,
                "test[{}] {}",
                i,
                d.name
            );
        }
    }
}
