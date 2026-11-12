// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::VhostUserConfig;
use crate::{
    device::{
        topology::PCIeTopology,
        util::{do_decrease_count, do_increase_count},
        Device, DeviceType,
    },
    Hypervisor as hypervisor,
};

#[derive(Debug, Clone, Default)]
pub struct VhostUserBlkDevice {
    pub device_id: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// driver type for block device
    pub driver_option: String,

    pub attach_count: u64,
    pub config: VhostUserConfig,
}

impl VhostUserBlkDevice {
    // new creates a new VhostUserBlkDevice
    pub fn new(device_id: String, config: VhostUserConfig) -> Self {
        VhostUserBlkDevice {
            device_id,
            attach_count: 0,
            config,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Device for VhostUserBlkDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        // increase attach count, skip attach the device if the device is already attached
        if self
            .increase_attach_count()
            .await
            .context("failed to increase attach count")?
        {
            return Ok(());
        }

        if let Err(e) = h.add_device(DeviceType::VhostUserBlk(self.clone())).await {
            self.decrease_attach_count().await?;

            return Err(e);
        }

        return Ok(());
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        // get the count of device detached, and detach once it reaches 0
        if self
            .decrease_attach_count()
            .await
            .context("failed to decrease attach count")?
        {
            return Ok(None);
        }

        if let Err(e) = h
            .remove_device(DeviceType::VhostUserBlk(self.clone()))
            .await
        {
            self.increase_attach_count().await?;

            return Err(e);
        }

        Ok(Some(self.config.index))
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for vhost-user-blk
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::VhostUserBlk(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        do_increase_count(&mut self.attach_count)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        do_decrease_count(&mut self.attach_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VhostUserType;

    fn test_config(socket_path: &str) -> VhostUserConfig {
        VhostUserConfig {
            dev_id: "vhu-blk-test".to_string(),
            socket_path: socket_path.to_string(),
            device_type: VhostUserType::Blk,
            driver_option: "virtio-blk-pci".to_string(),
            index: 3,
            queue_size: 256,
            num_queues: 2,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_new_vhost_user_blk_device() {
        let config = test_config("/run/vhu/vhost-blk-rawdisk01.sock");
        let device = VhostUserBlkDevice::new("device-id".to_string(), config.clone());

        assert_eq!(device.device_id, "device-id");
        assert_eq!(device.attach_count, 0);
        // These are only set by the callers that care about them; a freshly
        // created device must default to read-write and keep the host file open.
        assert!(!device.is_readonly);
        assert!(!device.no_drop);
        assert!(device.driver_option.is_empty());

        assert_eq!(device.config.dev_id, config.dev_id);
        assert_eq!(device.config.socket_path, config.socket_path);
        assert_eq!(device.config.index, config.index);
        assert_eq!(device.config.queue_size, config.queue_size);
        assert_eq!(device.config.num_queues, config.num_queues);
        assert!(matches!(
            device.config.device_type,
            VhostUserType::Blk
        ));
    }

    #[tokio::test]
    async fn test_vhost_user_blk_device_info_round_trip() {
        let config = test_config("/run/vhu/vhost-blk-rawdisk02.sock");
        let device = VhostUserBlkDevice::new("device-id".to_string(), config.clone());

        match device.get_device_info().await {
            DeviceType::VhostUserBlk(info) => {
                assert_eq!(info.device_id, "device-id");
                assert_eq!(info.config.socket_path, config.socket_path);
                assert_eq!(info.config.index, config.index);
            }
            other => panic!("unexpected device type: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_vhost_user_blk_attach_count() {
        let mut device =
            VhostUserBlkDevice::new("device-id".to_string(), test_config("/run/vhu/x.sock"));

        // The first attach must report that the device still has to be plugged
        // into the guest, every further attach is a no-op for the hypervisor.
        assert!(!device.increase_attach_count().await.unwrap());
        assert!(device.increase_attach_count().await.unwrap());
        assert_eq!(device.attach_count, 2);

        // The device is still in use until the last reference is dropped.
        assert!(device.decrease_attach_count().await.unwrap());
        assert!(!device.decrease_attach_count().await.unwrap());
        assert_eq!(device.attach_count, 0);

        // Detaching a device that is not attached is an error, not a panic.
        let err = device.decrease_attach_count().await.unwrap_err();
        assert!(
            err.to_string().contains("not attached"),
            "unexpected error: {err}"
        );
        assert_eq!(device.attach_count, 0);
    }
}
