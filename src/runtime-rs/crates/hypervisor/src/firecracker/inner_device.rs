//Copyright (c) 2019-2022 Alibaba Cloud
//Copyright (c) 2019-2022 Ant Group
//Copyright (c) 2023 Nubificus Ltd
//
//SPDX-License-Identifier: Apache-2.0

use super::FcInner;
use crate::device::util::get_virt_drive_name;
use crate::firecracker::{
    inner_hypervisor::{FC_AGENT_SOCKET_NAME, ROOT},
    sl,
};
use crate::VmmState;
use crate::{device::DeviceType, HybridVsockConfig, VsockConfig};
use anyhow::{anyhow, Context, Result};
use serde_json::json;

impl FcInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<()> {
        if self.state == VmmState::NotReady {
            info!(sl(), "VMM not ready, queueing device {}", device);

            self.pending_devices.insert(0, device);

            return Ok(());
        }

        debug!(sl(), "Add Device {} ", &device);

        match device {
            DeviceType::BlockModern(block_mod) => {
                let (path, slot, virt_path, index, is_readonly) = {
                    let mut block = block_mod.lock().await;
                    let index = block.config.index;
                    let is_readonly = block.config.is_readonly;
                    let slot = self.drive_slot_for_block_device(index, is_readonly)?;
                    let guest_device = get_virt_drive_name(slot as i32)?;
                    block.config.virt_path = format!("/dev/{guest_device}");
                    (
                        block.config.path_on_host.clone(),
                        slot,
                        block.config.virt_path.clone(),
                        index,
                        is_readonly,
                    )
                };
                if let Err(error) = self.hotplug_block_device(path.as_str(), slot).await {
                    self.release_block_drive_slot(index, is_readonly);
                    return Err(error).context("add block device");
                }
                debug!(sl!(), "Firecracker block device mapped to {}", virt_path);
                Ok(())
            }
            DeviceType::Network(network) => self
                .add_net_device(&network.config, network.device_id)
                .await
                .context("add net device"),
            DeviceType::HybridVsock(hvsock) => {
                self.add_hvsock(&hvsock.config).await.context("add vsock")
            }
            DeviceType::Vsock(vsock) => self.add_vsock(&vsock.config).await.context("add vsock"),
            _ => Err(anyhow!("unhandled device: {:?}", device)),
        }
    }

    // Since Firecracker doesn't support sharefs, we patch block devices on pre-start inserted
    // dummy drives
    pub(crate) async fn hotplug_block_device(&mut self, path: &str, id: u64) -> Result<()> {
        if id > 0 {
            self.patch_container_rootfs(&id.to_string(), path).await?;
        }
        Ok(())
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl(), "Remove Device {} ", device);
        if let DeviceType::BlockModern(block_mod) = device {
            let block = block_mod.lock().await;
            self.release_block_drive_slot(block.config.index, block.config.is_readonly);
        }
        Ok(())
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl(), "update device {:?}", &device);
        Ok(())
    }

    pub(crate) async fn add_hvsock(&mut self, config: &HybridVsockConfig) -> Result<()> {
        let rel_uds_path = match self.jailed {
            false => [self.vm_path.as_str(), FC_AGENT_SOCKET_NAME].join("/"),
            true => FC_AGENT_SOCKET_NAME.to_string(),
        };
        let body_vsock: String = json!({
            "vsock_id": String::from(ROOT),
            "guest_cid": config.guest_cid,
            "uds_path": rel_uds_path,
        })
        .to_string();

        info!(sl(), "HybridVsock configure: {:?}", &body_vsock);

        self.request_with_retry(hyper::Method::PUT, "/vsock", body_vsock)
            .await?;
        Ok(())
    }

    pub(crate) async fn add_vsock(&mut self, config: &VsockConfig) -> Result<()> {
        let rel_uds_path = match self.jailed {
            false => [self.vm_path.as_str(), FC_AGENT_SOCKET_NAME].join("/"),
            true => FC_AGENT_SOCKET_NAME.to_string(),
        };
        let body_vsock: String = json!({
            "vsock_id": String::from(ROOT),
            "guest_cid": config.guest_cid,
            "uds_path": rel_uds_path,
        })
        .to_string();

        info!(sl(), "HybridVsock configure: {:?}", &body_vsock);

        self.request_with_retry(hyper::Method::PUT, "/vsock", body_vsock)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockConfigModern, BlockDeviceModernHandle};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn remove_device_only_releases_existing_mapping() {
        for is_readonly in [false, true] {
            let (sender, _receiver) = mpsc::channel(1);
            let mut fc = FcInner::new(sender);
            let block = BlockDeviceModernHandle::new(
                "test-disk".to_string(),
                BlockConfigModern {
                    index: 1,
                    is_readonly,
                    ..Default::default()
                },
            );
            // Removing an unmapped device must succeed even with an exhausted pool.
            for index in 2..=9 {
                let _ = fc.drive_slot_for_block_device(index, is_readonly);
            }
            let readonly_before = fc.readonly_drive_slots.clone();
            let writable_before = fc.writable_drive_slots.clone();
            fc.remove_device(DeviceType::BlockModern(block.arc()))
                .await
                .unwrap();
            assert_eq!(fc.readonly_drive_slots, readonly_before);
            assert_eq!(fc.writable_drive_slots, writable_before);

            fc.release_block_drive_slot(2, is_readonly);
            let slot = fc.drive_slot_for_block_device(1, is_readonly).unwrap();
            fc.remove_device(DeviceType::BlockModern(block.arc()))
                .await
                .unwrap();
            assert_eq!(
                fc.drive_slot_for_block_device(10, is_readonly).unwrap(),
                slot
            );
        }
    }
}
